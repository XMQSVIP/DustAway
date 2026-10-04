# Fixed, previously observed profile only. Metadata diagnosis, never deletion or authorization.
$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
$profile=Join-Path $env:LOCALAPPDATA 'DingTalk_133\Default'
$locations=@(foreach($relative in @('','Cache','Code Cache','Local Storage','IndexedDB','GPUCache')){
 $path=if($relative){Join-Path $profile $relative}else{$profile}
 $item=Get-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
 @{location=if($relative){$relative}else{'Known Default profile'};exists=($null -ne $item);attributes=if($item){[string]$item.Attributes}else{$null}}
})
$present=@(foreach($view in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)){
 foreach($hive in @([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryHive]::LocalMachine)){
  $base=[Microsoft.Win32.RegistryKey]::OpenBaseKey($hive,$view)
  try{$key=$base.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Uninstall\DingTalk');if($key){$key.Dispose();[string]$hive+'/'+[string]$view}}finally{$base.Dispose()}
 }
})
@{time=(Get-Date -Format o);evidence_kind='fixed_location_metadata_only_not_cleanup_authority';registration_present=$present;main_executable_exists=(Test-Path -LiteralPath 'C:\Program Files (x86)\DingDing\main\current\DingTalk.exe');locations=$locations;deletion_executed=$false} | ConvertTo-Json -Depth 5
