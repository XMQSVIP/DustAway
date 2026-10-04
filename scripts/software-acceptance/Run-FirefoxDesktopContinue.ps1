$ErrorActionPreference='Stop'
$taskRoot=$PSScriptRoot
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Expected ordinary token'}
if(Get-Process -Name firefox,Un_A,helper,FirefoxSetup -ErrorAction SilentlyContinue){throw 'Vendor process remains; do not stop it'}
if(Test-Path -LiteralPath (Join-Path $taskRoot 'firefox-real/application/firefox.exe')){throw 'Test browser remains'}
foreach($taskHive in @([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryHive]::LocalMachine)){
 foreach($taskView in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)){
  $taskBase=[Microsoft.Win32.RegistryKey]::OpenBaseKey($taskHive,$taskView)
  try {$taskUninstall=$taskBase.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Uninstall'); if($taskUninstall){
   try {foreach($taskName in $taskUninstall.GetSubKeyNames()){$taskKey=$taskUninstall.OpenSubKey($taskName); if($taskKey){try{if([string]$taskKey.GetValue('DisplayName') -match '(?i)Firefox'){throw 'Firefox registration remains'}}finally{$taskKey.Dispose()}}}}
   finally {$taskUninstall.Dispose()}
  }} finally {$taskBase.Dispose()}
 }
}
[pscustomobject]@{Time=(Get-Date).ToString('o');OrdinaryUser=$true;SessionId=(Get-Process -Id $PID).SessionId;RegistrationAbsent=$true;VendorProcessesAbsent=$true;UserReportedCompleted=$true} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-desktop-vendor-completed.json') -Encoding UTF8
$taskWorkspace=Join-Path $taskRoot 'firefox-desktop-real'
$taskTarget=Join-Path $taskWorkspace 'scripts/software-acceptance/firefox-desktop-continue.mjs'
if(Test-Path -LiteralPath $taskTarget){throw 'Continuation already copied; do not replay'}
Copy-Item -LiteralPath (Join-Path $taskRoot 'firefox-desktop-continue.mjs') -Destination $taskTarget
$taskRun=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('scripts/software-acceptance/firefox-desktop-continue.mjs') -WorkingDirectory $taskWorkspace -WindowStyle Hidden -PassThru -Wait -RedirectStandardOutput (Join-Path $taskRoot 'firefox-desktop-continue-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-desktop-continue-error.log')
[pscustomobject]@{ExitCode=$taskRun.ExitCode;Finished=(Get-Date).ToString('o');OrdinaryUser=$true;ReusedLiveTask=$true;UninstallInvoked=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-desktop-continue-result.json') -Encoding UTF8
