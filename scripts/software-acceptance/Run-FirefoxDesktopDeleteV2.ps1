$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
$taskWorkspace=Join-Path $taskRoot 'firefox-desktop-delete-20261001-v2'
if([IO.File]::ReadAllText((Join-Path $taskWorkspace '.dustaway-real-ui-test')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong bundle'}
if(Get-Process -Name firefox,Un_A,Au_,FirefoxSetup,dustaway,tauri-driver,msedgedriver -ErrorAction SilentlyContinue){throw 'Prior process remains'}
if(Get-NetTCPConnection -State Listen -LocalPort 4444,4445,19837 -ErrorAction SilentlyContinue){throw 'Test ports occupied'}
$taskFirstFailure=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-desktop-delete-20261001\evidence\status.json') -Raw | ConvertFrom-Json
if($taskFirstFailure.phase -ne 'failed' -or $taskFirstFailure.taskId -ne '9197f792-7327-46e5-afbe-c3045fd59c7c'){throw 'Unexpected prior cycle'}
$taskPrior=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-rules-finished.json') -Raw | ConvertFrom-Json
if($taskPrior.DedicatedProcesses -ne 0 -or $taskPrior.FirefoxRegistrations -ne 0 -or $taskPrior.DedicatedFirefoxInstalled -or $taskPrior.MisplacedFirefoxInstalled){throw 'Prior cycle not finished'}
$taskInstall=[IO.Path]::GetFullPath((Join-Path $taskRoot 'firefox-real\application'))
if((Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe')) -or (Test-Path -LiteralPath (Join-Path (Split-Path -Parent $taskRoot) 'firefox.exe'))){throw 'Prior installation remains'}
foreach($taskHive in @([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryHive]::LocalMachine)){
 foreach($taskView in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)){
  $taskBase=[Microsoft.Win32.RegistryKey]::OpenBaseKey($taskHive,$taskView)
  try {
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
$taskProfile=[IO.Path]::GetFullPath((Join-Path $env:APPDATA ('Mozilla\Firefox\Profiles\'+$taskReady.ProfileDirectory)))
if([IO.File]::ReadAllText((Join-Path $taskProfile '.dustaway-test-only')) -ne 'DustAwayFirefox20260930'){throw 'Wrong profile marker'}
if(-not(Test-Path -LiteralPath (Join-Path $taskProfile 'user.js'))){throw 'Prior UI check unexpectedly deleted preferences'}
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-desktop-delete-v2-manifest.json') -Raw | ConvertFrom-Json
foreach($taskFile in $taskManifest.Files.PSObject.Properties){if($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name)).Hash -ne $taskFile.Value){throw 'Driver integrity mismatch'}}
if((Get-FileHash -LiteralPath (Join-Path $taskWorkspace 'target\debug\dustaway.exe')).Hash -ne '2CB798728C1E8173883D47545F92D8283B7BB0A53727CD14FB2D513CCDA754A1'){throw 'Unexpected standard application'}
$taskInstaller=Join-Path $taskRoot 'FirefoxSetup.exe'
if((Get-FileHash -LiteralPath $taskInstaller).Hash -ne 'DF6F7046D53FF17403E49092C0FE830E24CABD8DACBFAD3225870955908FCCF9'){throw 'Installer hash mismatch'}
$taskSignature=Get-AuthenticodeSignature -LiteralPath $taskInstaller
if($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'O=Mozilla Corporation'){throw 'Installer signature mismatch'}
$taskOnce=[IO.File]::Open((Join-Path $taskRoot 'firefox-desktop-delete-v2-run.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
$taskExit=99
try {
 $taskIni=Join-Path $taskRoot 'firefox-desktop-delete-v2-prepare.ini'
 if(Test-Path -LiteralPath $taskIni){throw 'Installer config already exists'}
 [IO.File]::WriteAllText($taskIni,('[Install]'+[Environment]::NewLine+'InstallDirectoryPath='+$taskInstall+[Environment]::NewLine+'MaintenanceService=false'+[Environment]::NewLine+'RegisterDefaultAgent=false'+[Environment]::NewLine+'DesktopShortcut=false'+[Environment]::NewLine+'StartMenuShortcuts=false'+[Environment]::NewLine+'TaskbarShortcut=false'+[Environment]::NewLine+'PreventRebootRequired=true'))
 $taskRun=Start-Process -FilePath $taskInstaller -ArgumentList ('/INI="'+$taskIni+'"') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru
 if($taskRun.ExitCode -ne 0 -or -not(Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe'))){throw 'INI installation not verified'}
 $taskVersion=[Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $taskInstall 'firefox.exe'))
 $taskNumericVersion='{0}.{1}.{2}.{3}' -f $taskVersion.FileMajorPart,$taskVersion.FileMinorPart,$taskVersion.FileBuildPart,$taskVersion.FilePrivatePart
 if($taskNumericVersion -ne '140.17.0.1158'){throw 'Version is not independently approved'}
 $taskHelper=Join-Path $taskInstall 'uninstall\helper.exe'
 foreach($taskExecutable in @((Join-Path $taskInstall 'firefox.exe'),$taskHelper)){
  $taskSig=Get-AuthenticodeSignature -LiteralPath $taskExecutable
  if($taskSig.Status -ne 'Valid' -or $taskSig.SignerCertificate.Subject -notmatch 'O=Mozilla Corporation'){throw 'Unexpected installed signature'}
 }
 $taskBrowser=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('firefox-profile-check.mjs','seed','firefox-desktop-delete-v2') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'firefox-desktop-delete-v2-browser-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-desktop-delete-v2-browser-error.log')
 if($taskBrowser.ExitCode -ne 0){throw 'Real browser data generation failed'}
 if(Get-Process -Name firefox,Un_A,Au_,FirefoxSetup -ErrorAction SilentlyContinue){throw 'Browser/installer process remains'}
 # The official /S option is set only on this newly installed, marked test fixture.
 # Production discovery, execution and UI run unchanged; this is not wizard evidence.
 $taskRegistrations=@()
 foreach($taskView in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)){
  $taskBase=[Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::CurrentUser,$taskView)
  try {
   $taskUninstall=$taskBase.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Uninstall')
   if($taskUninstall){try{foreach($taskName in $taskUninstall.GetSubKeyNames()){
    $taskKey=$taskUninstall.OpenSubKey($taskName)
    if($taskKey){try{if([string]$taskKey.GetValue('DisplayName') -match '(?i)Firefox'){
     $taskLocation=([string]$taskKey.GetValue('InstallLocation')).TrimEnd('\')
     $taskOriginal=[string]$taskKey.GetValue('UninstallString')
     if($taskLocation -ne $taskInstall -or $taskOriginal.Trim('"') -ne $taskHelper){throw 'Unexpected Firefox identity or uninstall command'}
     $taskRegistrations+= [pscustomobject]@{View=$taskView.ToString();Key=$taskName;OriginalUninstallString=$taskOriginal}
    }}finally{$taskKey.Dispose()}}
   }}finally{$taskUninstall.Dispose()}}
  }finally{$taskBase.Dispose()}
 }
 if($taskRegistrations.Count -lt 1 -or @($taskRegistrations.Key | Select-Object -Unique).Count -ne 1){throw 'Ambiguous fixture identity'}
 foreach($taskRegistration in $taskRegistrations){
  $taskBase=[Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::CurrentUser,[Enum]::Parse([Microsoft.Win32.RegistryView],$taskRegistration.View))
  try {
   $taskKey=$taskBase.OpenSubKey(('Software\Microsoft\Windows\CurrentVersion\Uninstall\'+$taskRegistration.Key),$true)
   try{
    if([string]$taskKey.GetValue('UninstallString') -notin @($taskRegistration.OriginalUninstallString,('"'+$taskHelper+'" /S'))){throw 'Registration changed during preparation'}
    $taskKey.SetValue('UninstallString',('"'+$taskHelper+'" /S'),[Microsoft.Win32.RegistryValueKind]::String)
   }finally{$taskKey.Dispose()}
  }finally{$taskBase.Dispose()}
 }
 [pscustomobject]@{Prepared=(Get-Date).ToString('o');OrdinaryUser=$true;Session=[Diagnostics.Process]::GetCurrentProcess().SessionId;ProfileDirectory=$taskReady.ProfileDirectory;SignedVersion=$taskNumericVersion;SilentFixtureRegistration=$true;OriginalRegistration=$taskRegistrations;VendorWizardInteractionTest=$false;AppSha256=(Get-FileHash -LiteralPath (Join-Path $taskWorkspace 'target\debug\dustaway.exe')).Hash} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-desktop-delete-v2-ready.json') -Encoding UTF8
 $taskCheck=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('scripts/software-acceptance/firefox-desktop-delete.mjs') -WorkingDirectory $taskWorkspace -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'firefox-desktop-delete-v2-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-desktop-delete-v2-error.log')
 $taskExit=$taskCheck.ExitCode
}catch{$_ | Out-String | Add-Content -LiteralPath (Join-Path $taskRoot 'firefox-desktop-delete-v2-error.log') -Encoding UTF8}
[pscustomobject]@{ExitCode=$taskExit;Finished=(Get-Date).ToString('o');OrdinaryUser=$true;ProductionProvider=$true;VendorWizardInteractionTest=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-desktop-delete-v2-result.json') -Encoding UTF8
exit $taskExit
