$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Expected ordinary token'}
$taskProof=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-residual-verification.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if($taskProof.result -ne 'passed' -or $taskProof.deleted_cache_files -ne 1 -or -not $taskProof.profile_data_unchanged){throw 'Residual check did not pass'}
if(Get-Process -Name firefox,Un_A,FirefoxSetup,software_firefox_acceptance_test -ErrorAction SilentlyContinue){throw 'Prior test process still active'}
$taskInstall=Join-Path $taskRoot 'firefox-real/application'
if(Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe')){throw 'Firefox remains installed; no reinstall'}
$taskProfileRoot=Join-Path $env:APPDATA 'Mozilla/Firefox'
$taskReady=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-real-ready.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$taskProfilePrefix='Profiles\'+$taskReady.ProfileDirectory+'\'
function Assert-ProtectedHashes {
 foreach($taskProperty in $taskProof.protected_hashes.PSObject.Properties){
  # Installer-owned profile registration files may change during reinstall;
  # compare every file within the marked user profile before reopening Firefox.
  if(-not $taskProperty.Name.StartsWith($taskProfilePrefix,[StringComparison]::OrdinalIgnoreCase)){continue}
  $taskPath=[IO.Path]::GetFullPath((Join-Path $taskProfileRoot $taskProperty.Name))
  if(-not $taskPath.StartsWith([IO.Path]::GetFullPath($taskProfileRoot)+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Invalid protected manifest path'}
  if((Get-FileHash -LiteralPath $taskPath -Algorithm SHA256).Hash -ne $taskProperty.Value){throw 'Protected profile file changed before browser reopen'}
 }
}
Assert-ProtectedHashes
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'installer-verification.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$taskInstaller=Join-Path $taskRoot 'FirefoxSetup.exe'
if((Get-FileHash -LiteralPath $taskInstaller -Algorithm SHA256).Hash -ne $taskManifest.Sha256){throw 'Installer hash differs'}
$taskSignature=Get-AuthenticodeSignature -LiteralPath $taskInstaller
if($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'O=Mozilla Corporation'){throw 'Unexpected installer signature'}
$taskLock=[IO.File]::Open((Join-Path $taskRoot 'firefox-reinstall.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskLock.Dispose()
$taskExit=99
try {
 $taskRun=Start-Process -FilePath $taskInstaller -ArgumentList @('/S',('/InstallDirectoryPath="'+$taskInstall+'"'),'/MaintenanceService=false','/RegisterDefaultAgent=false','/DesktopShortcut=false','/StartMenuShortcuts=false','/TaskbarShortcut=false','/PreventRebootRequired=true') -WorkingDirectory $taskRoot -WindowStyle Hidden -PassThru -Wait
 if($taskRun.ExitCode -ne 0){throw 'Reinstall failed'}
 Assert-ProtectedHashes
 $taskBrowser=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('firefox-profile-check.mjs','reinstalled') -WorkingDirectory $taskRoot -WindowStyle Hidden -PassThru -Wait -RedirectStandardOutput (Join-Path $taskRoot 'firefox-reinstalled-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-reinstalled-error.log')
 if($taskBrowser.ExitCode -ne 0){throw 'Reinstalled browser failed preserved-data check'}
 # Fixture teardown only. The acceptance uninstall above used the interactive production launcher.
 $taskHelper=Join-Path $taskInstall 'uninstall/helper.exe'
 $taskHelperSignature=Get-AuthenticodeSignature -LiteralPath $taskHelper
 if($taskHelperSignature.Status -ne 'Valid' -or $taskHelperSignature.SignerCertificate.Subject -notmatch 'O=Mozilla Corporation'){throw 'Unexpected cleanup uninstaller'}
 $taskCleanup=Start-Process -FilePath $taskHelper -ArgumentList @('/S') -WorkingDirectory $taskInstall -WindowStyle Hidden -PassThru -Wait
 if($taskCleanup.ExitCode -ne 0 -or (Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe'))){throw 'Test application teardown incomplete'}
 [pscustomobject]@{Passed=$true;Version=$taskManifest.Version;PreservedProfileHashesCheckedBeforeBrowserReopen=$true;CookieAndLocalStorageRestored=$true;OfficialSilentUninstallUsedOnlyForTestTeardown=$true;ApplicationBinaryRemoved=$true;TestDataRetained=$true;ProductionResidualDeletionEnabled=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-reinstall-verification.json') -Encoding UTF8
 $taskExit=0
} catch {$_ | Out-String | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-reinstall-error.log') -Encoding UTF8}
[pscustomobject]@{ExitCode=$taskExit;Finished=(Get-Date).ToString('o')} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-reinstall-result.json') -Encoding UTF8
