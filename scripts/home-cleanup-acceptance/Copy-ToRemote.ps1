param([Parameter(Mandatory=$true)][string[]]$Paths)
$ErrorActionPreference='Stop'
$copyArgs=@('-F','/dev/null','-i',(Join-Path $env:USERPROFILE '.ssh/dustaway-desktop-24njtt6-ed25519').Replace('\','/'),'-o','IdentitiesOnly=yes','-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o',('UserKnownHostsFile='+(Join-Path $env:USERPROFILE '.ssh/dustaway-desktop-24njtt6-known-hosts').Replace('\','/')),'-o','GlobalKnownHostsFile=/dev/null','-o','HostKeyAlgorithms=ssh-ed25519')
$sources=@($Paths | ForEach-Object {$path=(Resolve-Path -LiteralPath $_).Path.Replace('\','/');'/'+$path.Substring(0,1).ToLower()+'/'+$path.Substring(3)})
& 'C:\Program Files\Git\usr\bin\scp.exe' @copyArgs @sources 'MSN@100.69.113.49:C:/Users/MSN/AppData/Local/DustAwayAcceptance/20261001-home-cleanup/'
if($LASTEXITCODE -ne 0){throw 'Transfer did not complete'}
