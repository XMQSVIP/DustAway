$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$registrations=@(foreach($hive in @([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryHive]::LocalMachine)){
 foreach($view in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)){
  $base=[Microsoft.Win32.RegistryKey]::OpenBaseKey($hive,$view)
  try{$key=$base.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Uninstall');if($key){try{foreach($name in $key.GetSubKeyNames()){$item=$key.OpenSubKey($name);if($item){try{if(([string]$item.GetValue('DisplayName')) -match 'DingTalk|DingDing'){[pscustomobject]@{hive=[string]$hive;view=[string]$view;name=$item.GetValue('DisplayName');version=$item.GetValue('DisplayVersion');location=$item.GetValue('InstallLocation')}}}finally{$item.Dispose()}}}}finally{$key.Dispose()}}}finally{$base.Dispose()}
 }
})
$locations=@((Join-Path $env:APPDATA 'DingTalk'),(Join-Path $env:ProgramFiles 'DingDing'),(Join-Path ${env:ProgramFiles(x86)} 'DingDing'))
$locations+=@(Get-ChildItem -LiteralPath $env:LOCALAPPDATA -Directory -Filter 'DingTalk*' | ForEach-Object {$_.FullName})
$state=@($locations | ForEach-Object {if(Test-Path -LiteralPath $_){Get-Item -LiteralPath $_ | Select-Object FullName,CreationTime,LastWriteTime,Attributes}})
$processes=@(Get-CimInstance Win32_Process | Where-Object {$_.Name -match 'DingTalk|DingDing|DingtalkLauncher' -or $_.ExecutablePath -match 'DingTalk|DingDing'} | Select-Object Name,ProcessId,ParentProcessId,SessionId,ExecutablePath)
[ordered]@{time=(Get-Date -Format o);registrations=$registrations;locations=$state;processes=$processes} | ConvertTo-Json -Depth 5
