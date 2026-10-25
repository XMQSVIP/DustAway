param([ValidateSet('Upload','Download')][string]$Mode,[Parameter(Mandatory=$true)][string[]]$Paths)
$ErrorActionPreference='Stop'
$key=Join-Path $env:USERPROFILE '.ssh/dustaway-desktop-24njtt6-ed25519'
$hostsFile=Join-Path $env:USERPROFILE '.ssh/dustaway-desktop-24njtt6-known-hosts'
$fingerprint=& 'C:\Program Files\Git\usr\bin\ssh-keygen.exe' -lf $hostsFile
if($fingerprint -notmatch [regex]::Escape('SHA256:qTe49uQ4Pc/szc7HFgm8ICcDMDO8laK/zLVPLhLsYIE')){throw 'Fingerprint mismatch'}
$options=@('-F','/dev/null','-i',$key.Replace('\','/'),'-o','IdentitiesOnly=yes','-o','BatchMode=yes','-o','ConnectTimeout=8','-o','StrictHostKeyChecking=yes','-o',('UserKnownHostsFile='+$hostsFile.Replace('\','/')),'-o','GlobalKnownHostsFile=/dev/null','-o','HostKeyAlgorithms=ssh-ed25519')
$remote='MSN@100.69.113.49:C:/Users/MSN/AppData/Local/DustAwayAcceptance/weixin-021/'
if($Mode -eq 'Upload') {
  $sources=@($Paths | ForEach-Object { $path=(Resolve-Path -LiteralPath $_).Path.Replace('\','/'); '/'+$path.Substring(0,1).ToLower()+'/'+$path.Substring(3) })
  & 'C:\Program Files\Git\usr\bin\scp.exe' @options @sources $remote
} else {
  foreach($name in $Paths) { if($name -notmatch '^[A-Za-z0-9_.-]+$'){throw 'Basename required'}; & 'C:\Program Files\Git\usr\bin\scp.exe' @options ($remote+$name) '/g/my_ai/DustAway/artifacts/weixin-021/'; if($LASTEXITCODE -ne 0){throw 'Download failed'} }
}
if($LASTEXITCODE -ne 0){throw 'Transfer failed'}
