# Metadata only. Never reads account databases, credentials, or document contents.
$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong environment'}
$apps=@(Get-CimInstance Win32_Process | Where-Object {$_.Name -match '^(DingTalk|BaiduNetdisk|Weixin)\.exe$'} | Select-Object Name,ExecutablePath,SessionId -Unique)
$identities=@(foreach($app in $apps){
 if(!$app.ExecutablePath -or $app.ExecutablePath.StartsWith($root,[StringComparison]::OrdinalIgnoreCase)){continue}
 $file=Get-Item -LiteralPath $app.ExecutablePath
 $sig=Get-AuthenticodeSignature -LiteralPath $file.FullName
 $fixed='{0}.{1}.{2}.{3}' -f $file.VersionInfo.FileMajorPart,$file.VersionInfo.FileMinorPart,$file.VersionInfo.FileBuildPart,$file.VersionInfo.FilePrivatePart
 [ordered]@{name=$app.Name;path=$file.FullName;version=$file.VersionInfo.FileVersion;fixed_version=$fixed;parent_attributes=[string](Get-Item -LiteralPath $file.DirectoryName).Attributes;sha256=(Get-FileHash -LiteralPath $file.FullName).Hash;signature=[string]$sig.Status;signer=$sig.SignerCertificate.Subject;session=$app.SessionId}
})
function Measure-Cache([string]$id,[string]$path){
 $bytes=[long]0;$count=0;$skipped=0;$errors=0
 $exists=Test-Path -LiteralPath $path
 if($exists){
  $pending=New-Object 'System.Collections.Generic.Stack[string]';$pending.Push($path)
  while($pending.Count){
   $next=$pending.Pop()
   try{
    if((Get-Item -LiteralPath $next).Attributes -band [IO.FileAttributes]::ReparsePoint){$skipped++;continue}
    foreach($item in Get-ChildItem -LiteralPath $next -Force -ErrorAction Stop){
     if($item.Attributes -band [IO.FileAttributes]::ReparsePoint){$skipped++;continue}
     if($item.PSIsContainer){$pending.Push($item.FullName)}else{$count++;$bytes+=$item.Length}
    }
   }catch{$errors++}
  }
 }
 [ordered]@{rule_id=$id;exists=$exists;files=$count;bytes=[string]$bytes;unread_locations=$errors;reparse_skipped=$skipped;candidate_scan=$false}
}
$caches=@(
 foreach($dir in Get-ChildItem -LiteralPath $env:LOCALAPPDATA -Directory -Filter 'DingTalk_*'){
  if($dir.Name -notmatch '^DingTalk_[0-9]+$' -or $dir.Attributes -band [IO.FileAttributes]::ReparsePoint){continue}
  Measure-Cache 'dingtalk.http' (Join-Path $dir.FullName 'Default\Cache\Cache_Data')
  Measure-Cache 'dingtalk.code' (Join-Path $dir.FullName 'Default\Code Cache')
 }
 foreach($entry in @(@('http','Cache\Cache_Data'),@('code','Code Cache'),@('gpu','GPUCache'))){Measure-Cache ('baidu_netdisk.'+$entry[0]) (Join-Path $env:APPDATA ('baidunetdisk\'+$entry[1]))}
 Measure-Cache 'weixin.logs' (Join-Path $env:APPDATA 'Tencent\xwechat\log')
)
$result=[ordered]@{time=(Get-Date -Format o);identities=$identities;cache_metadata=$caches;read_only=$true;contents_read=$false}
$json=$result | ConvertTo-Json -Depth 6
$json | Set-Content -LiteralPath (Join-Path $root ('installed-metadata-'+(Get-Date -Format 'HHmmss')+'.json')) -Encoding UTF8
$json
