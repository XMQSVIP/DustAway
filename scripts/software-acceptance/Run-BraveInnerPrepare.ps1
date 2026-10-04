$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
if($env:GoogleUpdateIsMachine -eq '1' -or $env:BraveSoftwareUpdateIsMachine -eq '1'){throw 'System install environment refused'}
$taskOld=Get-Content -LiteralPath (Join-Path $taskRoot 'brave-prepare-result.json') -Raw | ConvertFrom-Json
if($taskOld.ExitCode -ne 99 -or -not ([IO.File]::ReadAllText((Join-Path $taskRoot 'brave-prepare-error.log')).Contains('Installer failed: -2147218431'))){throw 'Expected original terminal failure missing'}
if(@(Get-CimInstance Win32_Process | Where-Object {$_.Name -match '^(brave|BraveBrowserStandaloneSilentSetup|BraveUpdate|BraveUpdater|setup|brave-inner-installer-154.1.96.60)\.exe$'}).Count){throw 'Prior process remains; do not restart'}
if((Get-ScheduledTask -TaskName 'DustAway-Brave-Prepare-20261001').State -ne 'Ready'){throw 'Original task not terminal'}
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
$taskBase=Join-Path $env:LOCALAPPDATA 'BraveSoftware'
if([IO.File]::ReadAllText((Join-Path $taskBase '.dustaway-test-only')) -ne 'DustAwayBrave20261001'){throw 'Wrong product marker'}
foreach($taskStart in @($taskRoot,$taskBase)){
 for($taskParent=Get-Item -LiteralPath $taskStart; $null -ne $taskParent; $taskParent=$taskParent.Parent){if($taskParent.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Linked parent refused'}}
}
$taskChildren=@(Get-ChildItem -LiteralPath $taskBase -Force)
if($taskChildren.Count -ne 1 -or $taskChildren[0].Name -ne '.dustaway-test-only' -or $taskChildren[0].PSIsContainer -or ($taskChildren[0].Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Unexpected existing product files'}
foreach($taskName in @('brave-inner-prepare.started','brave-inner-prepare-result.json','brave-real-ready.json','brave-inner-install-preferences.json')){if(Test-Path -LiteralPath (Join-Path $taskRoot $taskName)){throw 'New preparation already attempted'}}
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'brave-inner-prepare-manifest.json') -Raw | ConvertFrom-Json
foreach($taskFile in $taskManifest.Files.PSObject.Properties){if($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name)).Hash -ne $taskFile.Value){throw 'Preparation integrity mismatch'}}
$taskInstaller=Join-Path $taskRoot 'brave-inner-installer-154.1.96.60.exe'
if((Get-FileHash -LiteralPath $taskInstaller).Hash -ne '6BF976C55EE239065ACC5702035D590EAF6A85B6E92EF7724B79A872E098A66E'){throw 'Inner installer hash mismatch'}
$taskSig=Get-AuthenticodeSignature -LiteralPath $taskInstaller
if($taskSig.Status -ne 'Valid' -or $taskSig.SignerCertificate.Subject -notmatch 'Brave Software, Inc\.'){throw 'Inner installer signature mismatch'}
$taskOnce=[IO.File]::Open((Join-Path $taskRoot 'brave-inner-prepare.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
$taskExit=99
$taskNativeExit=$null
try{
 $taskPrefs=Join-Path $taskRoot 'brave-inner-install-preferences.json'
 [ordered]@{distribution=@{system_level=$false;do_not_launch_chrome=$true;do_not_register_for_update_launch=$true;make_chrome_default_for_user=$false;do_not_create_any_shortcuts=$true;import_bookmarks=$false;import_history=$false;import_search_engine=$false;import_home_page=$false}} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $taskPrefs -Encoding ASCII
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
 if($taskHandle -eq [IntPtr]::Zero){throw 'Original installer handle unavailable'}
 [pscustomobject]@{Pid=$taskRun.Id;Started=(Get-Date).ToString('o');Stage='inner_installer_started';OrdinaryUser=$true;OriginalWrapperFailed=$true} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'brave-inner-prepare-status.json') -Encoding UTF8
 $taskRun.WaitForExit()
 $taskNativeExit=$taskRun.ExitCode
 if($null -eq $taskNativeExit -or $taskNativeExit -ne 0){throw ('Inner installer failed: '+$taskNativeExit)}
 $taskExe=Join-Path $taskBase 'Brave-Browser\Application\brave.exe'
 $taskVersion=[Diagnostics.FileVersionInfo]::GetVersionInfo($taskExe)
 $taskNumericVersion='{0}.{1}.{2}.{3}' -f $taskVersion.FileMajorPart,$taskVersion.FileMinorPart,$taskVersion.FileBuildPart,$taskVersion.FilePrivatePart
 if($taskNumericVersion -ne '154.1.96.60'){throw 'Unexpected installed version'}
 $taskSig=Get-AuthenticodeSignature -LiteralPath $taskExe
 if($taskSig.Status -ne 'Valid' -or $taskSig.SignerCertificate.Subject -notmatch 'Brave Software, Inc\.'){throw 'Unexpected installed signature'}
 if(Get-Process -Name brave -ErrorAction SilentlyContinue){throw 'Unexpected browser launch; inspect original process'}
 [pscustomobject]@{OrdinaryUser=$true;SignedVersion=$taskNumericVersion;BrowserSha256=(Get-FileHash -LiteralPath $taskExe).Hash;BaseMarker='DustAwayBrave20261001';ReadyForSeed=$true;Preparation='brave-inner-prepare';OriginalWrapperFailed=$true;DeletionAttempted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'brave-real-ready.json') -Encoding UTF8
 $taskExit=0
}catch{$_ | Out-String | Add-Content -LiteralPath (Join-Path $taskRoot 'brave-inner-prepare-error.log') -Encoding UTF8}
[pscustomobject]@{ExitCode=$taskExit;NativeExitCode=$taskNativeExit;Finished=(Get-Date).ToString('o');OrdinaryUser=$true;OriginalWrapperFailed=$true;DeletionAttempted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'brave-inner-prepare-result.json') -Encoding UTF8
exit $taskExit
