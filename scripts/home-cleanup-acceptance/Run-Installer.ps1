param([Parameter(Mandatory=$true)][ValidateSet('DingTalk','BaiduNetdisk','Weixin')][string]$Product)
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
$root=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $root -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup') -or [IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong test environment'}
$principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary interactive user required'}
$patterns=@{DingTalk='DingTalk|钉钉';BaiduNetdisk='BaiduNetdisk|百度网盘';Weixin='Weixin|WeChat|微信'}
$paths=@{
 DingTalk=@((Join-Path $env:APPDATA 'DingTalk'),(Join-Path $env:ProgramFiles 'DingDing'),(Join-Path ${env:ProgramFiles(x86)} 'DingDing'))
 BaiduNetdisk=@((Join-Path $env:APPDATA 'baidunetdisk'),(Join-Path $env:LOCALAPPDATA 'BaiduNetdisk'),(Join-Path $env:ProgramFiles 'BaiduNetdisk'),(Join-Path ${env:ProgramFiles(x86)} 'BaiduNetdisk'))
 Weixin=@((Join-Path $env:APPDATA 'Tencent\xwechat'),(Join-Path $env:APPDATA 'Tencent\WeChat'),(Join-Path $env:LOCALAPPDATA 'Tencent\WeChat'),(Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'WeChat Files'),(Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'xwechat_files'),(Join-Path $env:ProgramFiles 'Tencent\Weixin'),(Join-Path ${env:ProgramFiles(x86)} 'Tencent\WeChat'))
}
if($Product -eq 'DingTalk' -and @(Get-ChildItem -LiteralPath $env:LOCALAPPDATA -Directory -Filter 'DingTalk*').Count){throw 'Existing DingTalk profile; leave it untouched'}
foreach($path in $paths[$Product]){if(Test-Path -LiteralPath $path){throw 'Existing installation or data; leave it untouched'}}
if(@(Get-Process | Where-Object {$_.ProcessName -match $patterns[$Product]}).Count){throw 'Existing application process'}
foreach($hive in @([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryHive]::LocalMachine)){
 foreach($view in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)){
  $base=[Microsoft.Win32.RegistryKey]::OpenBaseKey($hive,$view)
  try{$key=$base.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Uninstall');if($key){try{foreach($name in $key.GetSubKeyNames()){$item=$key.OpenSubKey($name);if($item){try{if([string]$item.GetValue('DisplayName') -match $patterns[$Product]){throw 'Existing software registration'}}finally{$item.Dispose()}}}}finally{$key.Dispose()}}}finally{$base.Dispose()}
 }
}
$entry=Get-Content -LiteralPath (Join-Path $root ($Product+'.manifest.json')) -Raw | ConvertFrom-Json
if($entry.file -ne ($Product+'.exe')){throw 'Unexpected installer file'}
$installer=Join-Path $root $entry.file
if((Get-FileHash -LiteralPath $installer).Hash -ne $entry.sha256){throw 'Installer integrity mismatch'}
$signer=Get-AuthenticodeSignature -LiteralPath $installer
if($signer.Status -ne 'Valid' -or $signer.SignerCertificate.Subject -ne $entry.signer){throw 'Installer signature mismatch'}
$once=[IO.File]::Open((Join-Path $root ($Product+'.install.started')),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None);$once.Dispose()
[ordered]@{time=(Get-Date -Format o);product=$Product;ordinary_user=$true;existing_data_absent=$true;installer_sha256=$entry.sha256;stage='starting_interactive_installer';deletion_attempted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root ($Product+'.install-preflight.json')) -Encoding UTF8
# The user needs this vendor window to install and sign in. No silent switches or passwords.
$process=Start-Process -FilePath $installer -WorkingDirectory $root -PassThru
$originalHandle=$process.Handle
[ordered]@{time=(Get-Date -Format o);product=$Product;pid=$process.Id;stage='interactive_installer_launched';deletion_attempted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root ($Product+'.install-launch.json')) -Encoding UTF8
