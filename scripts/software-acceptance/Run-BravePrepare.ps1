$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
if(Get-Process -Name brave,BraveBrowserStandaloneSilentSetup -ErrorAction SilentlyContinue){throw 'Prior vendor process remains'}
if($env:GoogleUpdateIsMachine -eq '1' -or $env:BraveSoftwareUpdateIsMachine -eq '1'){throw 'System install environment refused'}
$taskBase=Join-Path $env:LOCALAPPDATA 'BraveSoftware'
if(Test-Path -LiteralPath $taskBase){throw 'Existing Brave data or installation; do not touch'}
foreach($taskHive in @([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryHive]::LocalMachine)){
 foreach($taskView in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)){
  $taskRegistry=[Microsoft.Win32.RegistryKey]::OpenBaseKey($taskHive,$taskView)
  try{
   $taskKeys=$taskRegistry.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Uninstall')
   if($taskKeys){try{foreach($taskName in $taskKeys.GetSubKeyNames()){
    $taskKey=$taskKeys.OpenSubKey($taskName)
    if($taskKey){try{if([string]$taskKey.GetValue('DisplayName') -match '(?i)Brave'){throw 'Existing Brave registration'}}finally{$taskKey.Dispose()}}
   }}finally{$taskKeys.Dispose()}}
  }finally{$taskRegistry.Dispose()}
 }
}
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'brave-prepare-manifest.json') -Raw | ConvertFrom-Json
foreach($taskFile in $taskManifest.Files.PSObject.Properties){if($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name)).Hash -ne $taskFile.Value){throw 'Preparation integrity mismatch'}}
$taskInstaller=Join-Path $taskRoot 'BraveBrowserStandaloneSilentSetup.exe'
if((Get-FileHash -LiteralPath $taskInstaller).Hash -ne '1F420CC550CF70CCFE555802E1BA6AE2D4D901288C119D678790BC7AE5D8EAE8'){throw 'Installer hash mismatch'}
$taskSignature=Get-AuthenticodeSignature -LiteralPath $taskInstaller
if($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'Brave Software, Inc\.'){throw 'Installer signature mismatch'}
$taskOnce=[IO.File]::Open((Join-Path $taskRoot 'brave-prepare.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
$taskExit=99
try{
 New-Item -ItemType Directory -Path $taskBase -ErrorAction Stop | Out-Null
 [IO.File]::WriteAllText((Join-Path $taskBase '.dustaway-test-only'),'DustAwayBrave20261001')
 $taskPrefs=Join-Path $taskRoot 'brave-install-preferences.json'
 if(Test-Path -LiteralPath $taskPrefs){throw 'Preferences already exist'}
 [ordered]@{distribution=@{do_not_launch_chrome=$true;do_not_register_for_update_launch=$true;make_chrome_default_for_user=$false;do_not_create_any_shortcuts=$true;import_bookmarks=$false;import_history=$false;import_search_engine=$false;import_home_page=$false};session=@{restore_on_startup=1}} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $taskPrefs -Encoding ASCII
 [pscustomobject]@{Started=(Get-Date).ToString('o');OrdinaryUser=$true;NoPriorBraveRoot=$true;NoPriorBraveRegistration=$true;ProfileMarker='DustAwayBrave20261001';DeletionAttempted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'brave-prepare-context.json') -Encoding UTF8
 $taskRun=Start-Process -FilePath $taskInstaller -ArgumentList @('--do-not-launch-chrome','--do-not-register-for-update-launch',('--installerdata="'+$taskPrefs+'"')) -WorkingDirectory $taskRoot -WindowStyle Hidden -PassThru
 [pscustomobject]@{Pid=$taskRun.Id;Started=(Get-Date).ToString('o');Stage='installer_started'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'brave-prepare-status.json') -Encoding UTF8
 $taskRun.WaitForExit()
 if($taskRun.ExitCode -ne 0){throw ('Installer failed: '+$taskRun.ExitCode)}
 $taskExe=Join-Path $taskBase 'Brave-Browser\Application\brave.exe'
 $taskVersion=[Diagnostics.FileVersionInfo]::GetVersionInfo($taskExe)
 $taskNumericVersion='{0}.{1}.{2}.{3}' -f $taskVersion.FileMajorPart,$taskVersion.FileMinorPart,$taskVersion.FileBuildPart,$taskVersion.FilePrivatePart
 if($taskNumericVersion -ne '154.1.96.60'){throw 'Unexpected installed version'}
 $taskSig=Get-AuthenticodeSignature -LiteralPath $taskExe
 if($taskSig.Status -ne 'Valid' -or $taskSig.SignerCertificate.Subject -notmatch 'Brave Software, Inc\.'){throw 'Unexpected installed signature'}
 if(Get-Process -Name brave -ErrorAction SilentlyContinue){throw 'Unexpected browser launch; inspect original process'}
 [pscustomobject]@{OrdinaryUser=$true;SignedVersion=$taskNumericVersion;BrowserSha256=(Get-FileHash -LiteralPath $taskExe).Hash;BaseMarker='DustAwayBrave20261001';ReadyForSeed=$true;DeletionAttempted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'brave-real-ready.json') -Encoding UTF8
 $taskExit=0
}catch{$_ | Out-String | Add-Content -LiteralPath (Join-Path $taskRoot 'brave-prepare-error.log') -Encoding UTF8}
[pscustomobject]@{ExitCode=$taskExit;Finished=(Get-Date).ToString('o');OrdinaryUser=$true;DeletionAttempted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'brave-prepare-result.json') -Encoding UTF8
exit $taskExit
