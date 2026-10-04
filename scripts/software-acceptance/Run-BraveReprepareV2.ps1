$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
if($env:GoogleUpdateIsMachine -eq '1' -or $env:BraveSoftwareUpdateIsMachine -eq '1'){throw 'System install environment refused'}
if(Get-Process -Name brave,setup,brave-rules-driver,brave-rules-v2-driver,brave-inner-installer-154.1.96.60 -ErrorAction SilentlyContinue){throw 'Original process remains'}
$taskOld=Get-Content -LiteralPath (Join-Path $taskRoot 'brave-rules-failed-finished.json') -Raw|ConvertFrom-Json
if($taskOld.NativeExit -ne 101 -or -not $taskOld.StandardUninstallVerified -or $taskOld.ResidualDeletion -ne 0 -or -not $taskOld.OriginalTaskRemoved){throw 'Original task not settled'}
foreach($taskHive in @([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryHive]::LocalMachine)){
 foreach($taskView in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)){
  $taskRegistry=[Microsoft.Win32.RegistryKey]::OpenBaseKey($taskHive,$taskView)
  try{$taskKeys=$taskRegistry.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Uninstall');if($taskKeys){try{foreach($taskName in $taskKeys.GetSubKeyNames()){$taskKey=$taskKeys.OpenSubKey($taskName);if($taskKey){try{if([string]$taskKey.GetValue('DisplayName') -match '(?i)Brave'){throw 'Existing Brave registration'}}finally{$taskKey.Dispose()}}}}finally{$taskKeys.Dispose()}}}finally{$taskRegistry.Dispose()}
 }
}
$taskBase=Join-Path $env:LOCALAPPDATA 'BraveSoftware'
foreach($taskPath in @($taskBase,(Join-Path $taskBase 'Brave-Browser\User Data'),(Join-Path $taskBase 'Brave-Browser\User Data\Default'))){
 if([IO.File]::ReadAllText((Join-Path $taskPath '.dustaway-test-only')) -ne 'DustAwayBrave20261001'){throw 'Wrong test profile marker'}
 for($taskParent=Get-Item -LiteralPath $taskPath; $null -ne $taskParent; $taskParent=$taskParent.Parent){if($taskParent.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Linked parent refused'}}
}
if(Test-Path -LiteralPath (Join-Path $taskBase 'Brave-Browser\Application\brave.exe')){throw 'Browser binary remains'}
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'brave-reprepare-v2-manifest.json') -Raw|ConvertFrom-Json
foreach($taskFile in $taskManifest.Files.PSObject.Properties){if($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name)).Hash -ne $taskFile.Value){throw 'Integrity mismatch'}}
$taskInstaller=Join-Path $taskRoot 'brave-inner-installer-154.1.96.60.exe'
if((Get-FileHash -LiteralPath $taskInstaller).Hash -ne '6BF976C55EE239065ACC5702035D590EAF6A85B6E92EF7724B79A872E098A66E'){throw 'Installer hash mismatch'}
$taskSig=Get-AuthenticodeSignature -LiteralPath $taskInstaller
if($taskSig.Status -ne 'Valid' -or $taskSig.SignerCertificate.Subject -notmatch 'Brave Software, Inc\.'){throw 'Installer signature mismatch'}
$taskOnce=[IO.File]::Open((Join-Path $taskRoot 'brave-reprepare-v2.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
$taskExit=99
$taskNativeExit=$null
try{
 $taskPrefs=Join-Path $taskRoot 'brave-reprepare-v2-preferences.json'
 if(Test-Path -LiteralPath $taskPrefs){throw 'Preferences already exist'}
 [ordered]@{distribution=@{system_level=$false;do_not_launch_chrome=$true;do_not_register_for_update_launch=$true;make_chrome_default_for_user=$false;do_not_create_any_shortcuts=$true;import_bookmarks=$false;import_history=$false;import_search_engine=$false;import_home_page=$false}}|ConvertTo-Json -Depth 4|Set-Content -LiteralPath $taskPrefs -Encoding ASCII
 $taskInfo=New-Object Diagnostics.ProcessStartInfo
 $taskInfo.FileName=$taskInstaller
 $taskInfo.Arguments='--do-not-launch-chrome --do-not-register-for-update-launch --verbose-logging --installerdata="'+$taskPrefs+'"'
 $taskInfo.WorkingDirectory=$taskRoot
 $taskInfo.UseShellExecute=$false
 $taskInfo.CreateNoWindow=$true
 $taskRun=New-Object Diagnostics.Process
 $taskRun.StartInfo=$taskInfo
 if(-not $taskRun.Start()){throw 'Installer did not start'}
 $taskHandle=$taskRun.Handle
 if($taskHandle -eq [IntPtr]::Zero){throw 'Original process handle unavailable'}
 [pscustomobject]@{Pid=$taskRun.Id;Started=(Get-Date).ToString('o');OrdinaryUser=$true;PriorContextExpired=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $taskRoot 'brave-reprepare-v2-status.json') -Encoding UTF8
 $taskRun.WaitForExit()
 $taskNativeExit=$taskRun.ExitCode
 if($null -eq $taskNativeExit -or $taskNativeExit -ne 0){throw ('Installer failed: '+$taskNativeExit)}
 $taskExe=Join-Path $taskBase 'Brave-Browser\Application\brave.exe'
 $taskVersion=[Diagnostics.FileVersionInfo]::GetVersionInfo($taskExe)
 $taskNumericVersion='{0}.{1}.{2}.{3}' -f $taskVersion.FileMajorPart,$taskVersion.FileMinorPart,$taskVersion.FileBuildPart,$taskVersion.FilePrivatePart
 $taskSig=Get-AuthenticodeSignature -LiteralPath $taskExe
 if($taskNumericVersion -ne '154.1.96.60' -or $taskSig.Status -ne 'Valid' -or $taskSig.SignerCertificate.Subject -notmatch 'Brave Software, Inc\.' -or (Get-FileHash -LiteralPath $taskExe).Hash -ne 'B7995F5B0926F45804DC12D5EA8057E7DB760DCB6AF2F18565B21A43EE3AE560'){throw 'Unexpected installed binary'}
 if(Get-Process -Name brave -ErrorAction SilentlyContinue){throw 'Unexpected browser launch'}
 [pscustomobject]@{OrdinaryUser=$true;SignedVersion=$taskNumericVersion;BrowserSha256=(Get-FileHash -LiteralPath $taskExe).Hash;Preparation='brave-reprepare-v2';DeletionAttempted=$false}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $taskRoot 'brave-real-ready-v4.json') -Encoding UTF8
 $taskExit=0
}catch{$_|Out-String|Add-Content -LiteralPath (Join-Path $taskRoot 'brave-reprepare-v2-error.log') -Encoding UTF8}
[pscustomobject]@{ExitCode=$taskExit;NativeExitCode=$taskNativeExit;Finished=(Get-Date).ToString('o');OrdinaryUser=$true;DeletionAttempted=$false}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $taskRoot 'brave-reprepare-v2-result.json') -Encoding UTF8
exit $taskExit
