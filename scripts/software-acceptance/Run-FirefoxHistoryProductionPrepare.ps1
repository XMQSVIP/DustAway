$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
if(Get-Process -Name firefox,Un_A,Au_,FirefoxSetup,dustaway,tauri-driver,msedgedriver,geckodriver -ErrorAction SilentlyContinue){throw 'Prior process remains'}
if(Get-NetTCPConnection -State Listen -LocalPort 4444,4445,4446,19837,19838,2829 -ErrorAction SilentlyContinue){throw 'Test ports occupied'}
$taskPrior=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-rules-20261001-report.json') -Raw | ConvertFrom-Json
if(-not $taskPrior.passed -or -not $taskPrior.original_identity_absent -or $taskPrior.production_deletion_entry){throw 'Prior evidence cycle incomplete'}
$taskInstall=[IO.Path]::GetFullPath((Join-Path $taskRoot 'firefox-real\application'))
if(Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe')){throw 'Prior installation remains'}
foreach($taskHive in @([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryHive]::LocalMachine)){
 foreach($taskView in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)){
  $taskBase=[Microsoft.Win32.RegistryKey]::OpenBaseKey($taskHive,$taskView)
  try{
   $taskUninstall=$taskBase.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Uninstall')
   if($taskUninstall){try{foreach($taskName in $taskUninstall.GetSubKeyNames()){
    $taskKey=$taskUninstall.OpenSubKey($taskName)
    if($taskKey){try{if([string]$taskKey.GetValue('DisplayName') -match '(?i)Firefox'){throw 'Existing Firefox registration'}}finally{$taskKey.Dispose()}}
   }}finally{$taskUninstall.Dispose()}}
  }finally{$taskBase.Dispose()}
 }
}
$taskReady=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-real-ready.json') -Raw | ConvertFrom-Json
if([string]$taskReady.ProfileDirectory -notmatch '^[a-zA-Z0-9_-]+\.DustAwayAcceptance20260930$'){throw 'Wrong profile leaf'}
$taskProfile=Join-Path $env:APPDATA ('Mozilla\Firefox\Profiles\'+$taskReady.ProfileDirectory)
if([IO.File]::ReadAllText((Join-Path $taskProfile '.dustaway-test-only')) -ne 'DustAwayFirefox20260930'){throw 'Wrong profile marker'}
foreach($taskFolder in @('bookmarkbackups','sessionstore-backups')){
 if(@(Get-ChildItem -LiteralPath (Join-Path $taskProfile $taskFolder) -File -Filter '*.jsonlz4' -ErrorAction SilentlyContinue).Count){throw 'Existing browser backups require separate inspection'}
}
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-production-manifest.json') -Raw | ConvertFrom-Json
foreach($taskFile in $taskManifest.Files.PSObject.Properties){if($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name)).Hash -ne $taskFile.Value){throw 'Driver integrity mismatch'}}
$taskInstaller=Join-Path $taskRoot 'FirefoxSetup.exe'
if((Get-FileHash -LiteralPath $taskInstaller).Hash -ne 'DF6F7046D53FF17403E49092C0FE830E24CABD8DACBFAD3225870955908FCCF9'){throw 'Installer hash mismatch'}
$taskSignature=Get-AuthenticodeSignature -LiteralPath $taskInstaller
if($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'O=Mozilla Corporation'){throw 'Installer signature mismatch'}
$taskOnce=[IO.File]::Open((Join-Path $taskRoot 'firefox-history-production-prepare.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
$taskExit=99
try{
 $taskIni=Join-Path $taskRoot 'firefox-history-production-prepare.ini'
 if(Test-Path -LiteralPath $taskIni){throw 'Installer config already exists'}
 [IO.File]::WriteAllText($taskIni,('[Install]'+[Environment]::NewLine+'InstallDirectoryPath='+$taskInstall+[Environment]::NewLine+'MaintenanceService=false'+[Environment]::NewLine+'RegisterDefaultAgent=false'+[Environment]::NewLine+'DesktopShortcut=false'+[Environment]::NewLine+'StartMenuShortcuts=false'+[Environment]::NewLine+'TaskbarShortcut=false'+[Environment]::NewLine+'PreventRebootRequired=true'))
 $taskRun=Start-Process -FilePath $taskInstaller -ArgumentList ('/INI="'+$taskIni+'"') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru
 if($taskRun.ExitCode -ne 0 -or -not(Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe'))){throw 'INI installation not verified'}
 $taskVersion=[Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $taskInstall 'firefox.exe'))
 $taskNumericVersion='{0}.{1}.{2}.{3}' -f $taskVersion.FileMajorPart,$taskVersion.FileMinorPart,$taskVersion.FileBuildPart,$taskVersion.FilePrivatePart
 if($taskNumericVersion -ne '140.17.0.1158'){throw 'Unexpected version'}
 foreach($taskExecutable in @((Join-Path $taskInstall 'firefox.exe'),(Join-Path $taskInstall 'uninstall\helper.exe'))){
  $taskSig=Get-AuthenticodeSignature -LiteralPath $taskExecutable
  if($taskSig.Status -ne 'Valid' -or $taskSig.SignerCertificate.Subject -notmatch 'O=Mozilla Corporation'){throw 'Unexpected installed signature'}
 }
 [pscustomobject]@{Started=(Get-Date).ToString('o');OrdinaryUser=$true;SignedVersion=$taskNumericVersion;ProfileDirectory=$taskReady.ProfileDirectory;BrowserUiTestApiScopedToMarkedProfile=$true;WindowsSecuritySettingsChanged=$false;DeletionAttempted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-production-prepare-context.json') -Encoding UTF8
 $taskBrowser=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('firefox-history-seed.mjs','firefox-history-production') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'firefox-history-production-prepare-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-history-production-prepare-error.log')
 $taskExit=$taskBrowser.ExitCode
}catch{$_ | Out-String | Add-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-production-prepare-error.log') -Encoding UTF8}
[pscustomobject]@{ExitCode=$taskExit;Finished=(Get-Date).ToString('o');OrdinaryUser=$true;DeletionAttempted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-production-prepare-result.json') -Encoding UTF8
exit $taskExit
