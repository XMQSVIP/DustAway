$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
if(Get-Process -Name firefox,Un_A,Au_,FirefoxSetup,software_firefox_production_test,software_firefox_production_ini_test,dustaway -ErrorAction SilentlyContinue){throw 'Prior process remains'}
$taskRepair=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-location-repair-result.json') -Raw | ConvertFrom-Json
if(-not $taskRepair.Passed -or -not $taskRepair.MisplacedTestInstallationRemoved){throw 'Location repair incomplete'}
$taskInstall=[IO.Path]::GetFullPath((Join-Path $taskRoot 'firefox-real\application'))
if((Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe')) -or (Test-Path -LiteralPath (Join-Path (Split-Path -Parent $taskRoot) 'firefox.exe'))){throw 'Prior installation remains'}
$taskReady=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-real-ready.json') -Raw | ConvertFrom-Json
if([string]$taskReady.ProfileDirectory -notmatch '^[a-zA-Z0-9_-]+\.DustAwayAcceptance20260930$'){throw 'Wrong profile leaf'}
$taskProfile=[IO.Path]::GetFullPath((Join-Path $env:APPDATA ('Mozilla\Firefox\Profiles\'+$taskReady.ProfileDirectory)))
if([IO.File]::ReadAllText((Join-Path $taskProfile '.dustaway-test-only')) -ne 'DustAwayFirefox20260930'){throw 'Wrong profile marker'}
foreach($taskLeaf in @('prefs.js','user.js','cookies.sqlite','places.sqlite')){if(-not(Test-Path -LiteralPath (Join-Path $taskProfile $taskLeaf))){throw 'Prior production attempt progressed beyond reinstall preflight'}}
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-production-ini-manifest.json') -Raw | ConvertFrom-Json
foreach($taskFile in $taskManifest.Files.PSObject.Properties){if($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name)).Hash -ne $taskFile.Value){throw 'Driver integrity mismatch'}}
$taskInstaller=Join-Path $taskRoot 'FirefoxSetup.exe'
if((Get-FileHash -LiteralPath $taskInstaller).Hash -ne 'DF6F7046D53FF17403E49092C0FE830E24CABD8DACBFAD3225870955908FCCF9'){throw 'Installer hash mismatch'}
$taskSignature=Get-AuthenticodeSignature -LiteralPath $taskInstaller
if($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'O=Mozilla Corporation'){throw 'Installer signature mismatch'}
$taskOnce=[IO.File]::Open((Join-Path $taskRoot 'firefox-production-ini-run.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
[pscustomobject]@{Started=(Get-Date).ToString('o');Elevated=$false;Session=[Diagnostics.Process]::GetCurrentProcess().SessionId;NewContext=$true;InteractiveLauncherTest=$false;PriorFailedResultsRetained=$true} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-production-ini-context.json') -Encoding UTF8
$taskExit=99
try {
 $taskNewScope=[IO.Path]::GetFullPath((Join-Path $taskProfile 'sessionstore-backups'))
 if(-not $taskNewScope.StartsWith($taskProfile+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Scope escaped profile'}
 $taskChildren=@(Get-ChildItem -LiteralPath $taskNewScope -Force)
 if($taskChildren.Count -ne 1 -or $taskChildren[0].Name -ne '.dustaway-new-scope' -or [IO.File]::ReadAllText($taskChildren[0].FullName) -ne 'test-only scope created after preparation'){throw 'New-scope witness changed'}
 Remove-Item -LiteralPath $taskChildren[0].FullName
 [IO.Directory]::Delete($taskNewScope,$false)
 $taskIni=Join-Path $taskRoot 'firefox-production-ini-prepare.ini'
 if(Test-Path -LiteralPath $taskIni){throw 'Installer config already exists'}
 [IO.File]::WriteAllText($taskIni,('[Install]'+[Environment]::NewLine+'InstallDirectoryPath='+$taskInstall+[Environment]::NewLine+'MaintenanceService=false'+[Environment]::NewLine+'RegisterDefaultAgent=false'+[Environment]::NewLine+'DesktopShortcut=false'+[Environment]::NewLine+'StartMenuShortcuts=false'+[Environment]::NewLine+'TaskbarShortcut=false'+[Environment]::NewLine+'PreventRebootRequired=true'))
 $taskRun=Start-Process -FilePath $taskInstaller -ArgumentList ('/INI="'+$taskIni+'"') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru
 if($taskRun.ExitCode -ne 0 -or -not(Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe'))){throw 'INI installation not verified'}
 $taskBrowser=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('firefox-profile-check.mjs','retained','firefox-production-ini') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'firefox-production-ini-browser-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-production-ini-browser-error.log')
 if($taskBrowser.ExitCode -ne 0){throw 'Existing fixture data not verified'}
 $taskCheck=Start-Process -FilePath (Join-Path $taskRoot 'software_firefox_production_ini_test.exe') -ArgumentList @('--ignored','--exact','software::firefox_rules_acceptance::real_firefox_production_rules','--nocapture') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'firefox-production-ini-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-production-ini-error.log')
 $taskExit=$taskCheck.ExitCode
} catch {$_ | Out-String | Add-Content -LiteralPath (Join-Path $taskRoot 'firefox-production-ini-error.log')}
[pscustomobject]@{ExitCode=$taskExit;Finished=(Get-Date).ToString('o');InteractiveLauncherTest=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-production-ini-result.json') -Encoding UTF8
exit $taskExit
