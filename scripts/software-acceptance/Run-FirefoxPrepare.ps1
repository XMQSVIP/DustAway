$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Expected ordinary token'}
$taskInstall=Join-Path $taskRoot 'firefox-real/application'
$taskLocal=Join-Path $env:LOCALAPPDATA 'Mozilla'
$taskRoaming=Join-Path $env:APPDATA 'Mozilla'
if((Test-Path -LiteralPath $taskLocal) -or (Test-Path -LiteralPath $taskRoaming) -or (Test-Path -LiteralPath $taskInstall)){throw 'Pre-existing Firefox or Mozilla data; nothing installed'}
if(Get-Process -Name firefox -ErrorAction SilentlyContinue){throw 'Firefox already running'}
foreach($taskHive in @([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryHive]::LocalMachine)){
 foreach($taskView in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)){
  $taskBase=[Microsoft.Win32.RegistryKey]::OpenBaseKey($taskHive,$taskView)
  try {
   $taskUninstall=$taskBase.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Uninstall')
   if($taskUninstall){
    try { foreach($taskName in $taskUninstall.GetSubKeyNames()){
     $taskKey=$taskUninstall.OpenSubKey($taskName)
     if($taskKey){try{if([string]$taskKey.GetValue('DisplayName') -match '(?i)Firefox'){throw 'Existing Firefox registration; nothing installed'}}finally{$taskKey.Dispose()}}
    }} finally {$taskUninstall.Dispose()}
   }
  } finally {$taskBase.Dispose()}
 }
}
$taskInstaller=Join-Path $taskRoot 'FirefoxSetup.exe'
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'installer-verification.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if((Get-FileHash -LiteralPath $taskInstaller -Algorithm SHA256).Hash -ne $taskManifest.Sha256){throw 'Installer hash differs'}
$taskSignature=Get-AuthenticodeSignature -LiteralPath $taskInstaller
if($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'O=Mozilla Corporation'){throw 'Unexpected Mozilla signature'}
$taskLock=[IO.File]::Open((Join-Path $taskRoot 'firefox-real.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskLock.Dispose()
[pscustomobject]@{Stage='prepared_before_install';Account=[Security.Principal.WindowsIdentity]::GetCurrent().Name;Elevated=$false;Session=[Diagnostics.Process]::GetCurrentProcess().SessionId;Started=(Get-Date).ToString('o');MozillaDataAbsent=$true;FirefoxRegistrationAbsent=$true;InstallerSha256=$taskManifest.Sha256;Version=$taskManifest.Version} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-real-context.json') -Encoding UTF8
$taskExit=99
try {
 $taskRun=Start-Process -FilePath $taskInstaller -ArgumentList @('/S',('/InstallDirectoryPath="'+$taskInstall+'"'),'/MaintenanceService=false','/RegisterDefaultAgent=false','/DesktopShortcut=false','/StartMenuShortcuts=false','/TaskbarShortcut=false','/PreventRebootRequired=true') -WorkingDirectory $taskRoot -WindowStyle Hidden -PassThru -Wait
 if($taskRun.ExitCode -ne 0 -or -not (Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe'))){throw 'Installation failed'}
 $taskCreate=Start-Process -FilePath (Join-Path $taskInstall 'firefox.exe') -ArgumentList @('-CreateProfile','DustAwayAcceptance20260930') -WorkingDirectory $taskRoot -WindowStyle Hidden -PassThru -Wait
 if($taskCreate.ExitCode -ne 0){throw 'Profile creation failed'}
 $taskProfiles=@(Get-ChildItem -LiteralPath (Join-Path $taskRoaming 'Firefox/Profiles') -Directory)
 if($taskProfiles.Count -ne 1 -or $taskProfiles[0].Name -notlike '*.DustAwayAcceptance20260930'){throw 'Unexpected generated profiles'}
 [IO.File]::WriteAllText((Join-Path $taskProfiles[0].FullName '.dustaway-test-only'),'DustAwayFirefox20260930')
 [IO.File]::WriteAllText((Join-Path $taskProfiles[0].FullName 'user.js'),'user_pref("browser.shell.checkDefaultBrowser", false);'+[Environment]::NewLine+'user_pref("browser.startup.homepage_override.mstone", "ignore");')
 # A sibling witness demonstrates that the shared Mozilla parent is never in scope.
 [IO.File]::WriteAllText((Join-Path $taskRoaming 'dustaway-shared-witness.txt'),'must remain outside Firefox residual roots')
 $taskBrowser=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('firefox-profile-check.mjs','seed') -WorkingDirectory $taskRoot -WindowStyle Hidden -PassThru -Wait -RedirectStandardOutput (Join-Path $taskRoot 'firefox-seed-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-seed-error.log')
 if($taskBrowser.ExitCode -ne 0){throw 'Browser data generation failed'}
 [IO.File]::WriteAllText((Join-Path $taskRoot 'firefox-real-ready.json'),(@{ProfileDirectory=$taskProfiles[0].Name;Version=$taskManifest.Version;InstallDirectory=$taskInstall;Prepared=$true}|ConvertTo-Json))
 $taskExit=0
} catch {$_ | Out-String | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-prepare-error.log') -Encoding UTF8}
[pscustomobject]@{ExitCode=$taskExit;Finished=(Get-Date).ToString('o')} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-prepare-result.json') -Encoding UTF8
exit $taskExit
