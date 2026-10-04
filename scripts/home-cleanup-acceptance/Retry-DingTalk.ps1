# One user-requested retry of this session's official installer. Never removes data.
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $root -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup') -or [IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong environment'}
$principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary interactive user required'}
$prior=Get-Content -LiteralPath (Join-Path $root 'DingTalk.install-preflight.json') -Raw | ConvertFrom-Json
$launch=Get-Content -LiteralPath (Join-Path $root 'DingTalk.install-launch.json') -Raw | ConvertFrom-Json
if(!$prior.existing_data_absent -or $launch.product -ne 'DingTalk'){throw 'Original absence evidence missing'}
if(@(Get-Process | Where-Object {$_.ProcessName -match 'DingTalk|DingDing'}).Count){throw 'Installer or application still running'}
foreach($path in @((Join-Path $env:ProgramFiles 'DingDing'),(Join-Path ${env:ProgramFiles(x86)} 'DingDing'))){if(Test-Path -LiteralPath $path){throw 'Installed files now present; inspect before retry'}}
if(@(Get-ChildItem -LiteralPath $env:LOCALAPPDATA -Directory -Filter 'DingTalk*').Count){throw 'User profile now present; inspect before retry'}
$profile=Join-Path $env:APPDATA 'DingTalk'
if((Test-Path -LiteralPath $profile) -and ((Get-Item -LiteralPath $profile).CreationTimeUtc -lt ([DateTimeOffset]::Parse($prior.time)).UtcDateTime)){throw 'Profile predates test installation'}
foreach($hive in @([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryHive]::LocalMachine)){
 foreach($view in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)){
  $base=[Microsoft.Win32.RegistryKey]::OpenBaseKey($hive,$view)
  try{$key=$base.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Uninstall');if($key){try{foreach($name in $key.GetSubKeyNames()){$item=$key.OpenSubKey($name);if($item){try{if(([string]$item.GetValue('DisplayName')) -match 'DingTalk|DingDing|\u9489\u9489'){throw 'Installation registered; inspect before retry'}}finally{$item.Dispose()}}}}finally{$key.Dispose()}}}finally{$base.Dispose()}
 }
}
$installer=Join-Path $root 'DingTalk.exe'
$expected='C4664B74F7B688FCBE30B951632EB6B0ABB820703908835695B5F82F8431CF56'
$manifest=Get-Content -LiteralPath (Join-Path $root 'DingTalk.manifest.json') -Raw | ConvertFrom-Json
$signature=Get-AuthenticodeSignature -LiteralPath $installer
if((Get-FileHash -LiteralPath $installer).Hash -ne $expected -or $signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -ne $manifest.signer){throw 'Installer identity changed'}
$once=[IO.File]::Open((Join-Path $root 'DingTalk.retry.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None);$once.Dispose()
$process=Start-Process -FilePath $installer -WorkingDirectory $root -PassThru
$handle=$process.Handle
[ordered]@{time=(Get-Date -Format o);product='DingTalk';pid=$process.Id;sha256=$expected;stage='user_requested_interactive_retry';data_deleted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'DingTalk.retry-launch.json') -Encoding UTF8
