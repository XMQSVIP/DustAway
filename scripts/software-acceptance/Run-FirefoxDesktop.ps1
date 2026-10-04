$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Expected ordinary token'}
$taskWorkspace=Join-Path $taskRoot 'firefox-desktop-real'
if([IO.File]::ReadAllText((Join-Path $taskWorkspace '.dustaway-real-ui-test')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong bundle'}
$taskPrior=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-final-status.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if($taskPrior.Result -ne 'supplement_and_reinstall_passed_with_limited_scope' -or $taskPrior.TestApplicationRegistrationsRemaining -ne 0 -or $taskPrior.ProductionResidualDeletionEnabled){throw 'Prior phase incomplete'}
if(Get-Process -Name firefox,Un_A,FirefoxSetup,software_firefox_acceptance_test,dustaway,tauri-driver,msedgedriver -ErrorAction SilentlyContinue){throw 'A prior test or vendor process remains'}
if(Get-NetTCPConnection -State Listen -LocalPort 4444,4445,19837 -ErrorAction SilentlyContinue){throw 'Test ports occupied'}
$taskInstall=Join-Path $taskRoot 'firefox-real/application'
if(Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe')){throw 'Firefox remains installed'}
foreach($taskHive in @([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryHive]::LocalMachine)){
 foreach($taskView in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)){
  $taskBase=[Microsoft.Win32.RegistryKey]::OpenBaseKey($taskHive,$taskView)
  try {
   $taskUninstall=$taskBase.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Uninstall')
   if($taskUninstall){
    try {foreach($taskName in $taskUninstall.GetSubKeyNames()){
     $taskKey=$taskUninstall.OpenSubKey($taskName)
     if($taskKey){try{if([string]$taskKey.GetValue('DisplayName') -match '(?i)Firefox'){throw 'Existing Firefox registration'}}finally{$taskKey.Dispose()}}
    }} finally {$taskUninstall.Dispose()}
   }
  } finally {$taskBase.Dispose()}
 }
}
$taskProfile=Join-Path $env:APPDATA 'Mozilla/Firefox/Profiles/q27dwpy5.DustAwayAcceptance20260930'
if([IO.File]::ReadAllText((Join-Path $taskProfile '.dustaway-test-only')) -ne 'DustAwayFirefox20260930'){throw 'Profile marker missing'}
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'installer-verification.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$taskInstaller=Join-Path $taskRoot 'FirefoxSetup.exe'
if((Get-FileHash -LiteralPath $taskInstaller -Algorithm SHA256).Hash -ne $taskManifest.Sha256){throw 'Installer changed'}
$taskSignature=Get-AuthenticodeSignature -LiteralPath $taskInstaller
if($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'O=Mozilla Corporation'){throw 'Unexpected signature'}
$taskLock=[IO.File]::Open((Join-Path $taskRoot 'firefox-desktop-prepare.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskLock.Dispose()
$taskExit=99
try {
 [pscustomobject]@{Time=(Get-Date).ToString('o');OrdinaryUser=$true;SessionId=(Get-Process -Id $PID).SessionId;AppSha256=(Get-FileHash -LiteralPath (Join-Path $taskWorkspace 'target/debug/dustaway.exe') -Algorithm SHA256).Hash;FixtureProvider=$false;ProductionResidualDeletionEnabled=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-desktop-context.json') -Encoding UTF8
 $taskRun=Start-Process -FilePath $taskInstaller -ArgumentList @('/S',('/InstallDirectoryPath="'+$taskInstall+'"'),'/MaintenanceService=false','/RegisterDefaultAgent=false','/DesktopShortcut=false','/StartMenuShortcuts=false','/TaskbarShortcut=false','/PreventRebootRequired=true') -WorkingDirectory $taskRoot -WindowStyle Hidden -PassThru -Wait
 if($taskRun.ExitCode -ne 0 -or -not (Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe'))){throw 'Test installation failed'}
 $taskHelper=Join-Path $taskInstall 'uninstall/helper.exe'
 $taskHelperSignature=Get-AuthenticodeSignature -LiteralPath $taskHelper
 if($taskHelperSignature.Status -ne 'Valid' -or $taskHelperSignature.SignerCertificate.Subject -notmatch 'O=Mozilla Corporation'){throw 'Unexpected uninstaller signature'}
 [pscustomobject]@{ExitCode=0;OrdinaryUser=$true;InstallDirectory=$taskInstall;Version=$taskManifest.Version;HelperSignature='Mozilla Corporation';Time=(Get-Date).ToString('o')} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-desktop-prepare-result.json') -Encoding UTF8
 $taskCheck=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('scripts/software-acceptance/firefox-desktop-check.mjs') -WorkingDirectory $taskWorkspace -WindowStyle Hidden -PassThru -Wait -RedirectStandardOutput (Join-Path $taskRoot 'firefox-desktop-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-desktop-error.log')
 $taskExit=$taskCheck.ExitCode
} catch {$_ | Out-String | Add-Content -LiteralPath (Join-Path $taskRoot 'firefox-desktop-error.log') -Encoding UTF8}
[pscustomobject]@{ExitCode=$taskExit;Finished=(Get-Date).ToString('o');OrdinaryUser=$true;RealProvider=$true} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-desktop-result.json') -Encoding UTF8
