$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
$entries=@(foreach($hive in @([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryHive]::LocalMachine)){
 foreach($view in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)){
  $base=[Microsoft.Win32.RegistryKey]::OpenBaseKey($hive,$view)
  try{$un=$base.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Uninstall');if($un){try{foreach($name in $un.GetSubKeyNames()){
   $key=$un.OpenSubKey($name)
   try{if([string]$key.GetValue('DisplayName') -match '^(钉钉|DingTalk)$'){
    @{hive=[string]$hive;view=[string]$view;key=$name;name=$key.GetValue('DisplayName');version=$key.GetValue('DisplayVersion');location=$key.GetValue('InstallLocation');command=$key.GetValue('UninstallString');publisher=$key.GetValue('Publisher')}
   }}finally{if($key){$key.Dispose()}}
  }}finally{$un.Dispose()}}}finally{$base.Dispose()}
 }
})
$old=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software\firefox-desktop-delete-20261001-v2'
$files=@(foreach($path in @('C:\Program Files (x86)\DingDing\main\current\DingTalk.exe',(Join-Path $old 'bin\tauri-driver.exe'),(Join-Path $old 'artifacts\webdriver\msedgedriver.exe'),'D:\nodejs\node.exe')){
 if(Test-Path -LiteralPath $path){@{path=$path;sha256=(Get-FileHash -LiteralPath $path).Hash;version=(Get-Item -LiteralPath $path).VersionInfo.FileVersion}}
})
$result=@{time=(Get-Date -Format o);entries=$entries;files=$files;processes=@(Get-CimInstance Win32_Process | Where-Object {$_.Name -match 'DingTalk|DingDing|dustaway|tauri-driver|msedgedriver'} | Select-Object Name,ProcessId,SessionId,ExecutablePath);listeners=@(Get-NetTCPConnection -State Listen -LocalPort 4444,4445,19837 -ErrorAction SilentlyContinue | Select-Object LocalPort,OwningProcess);uninstall_started=$false}
$result | ConvertTo-Json -Depth 5
