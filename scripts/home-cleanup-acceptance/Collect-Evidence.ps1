param([Parameter(Mandatory=$true)][string]$Manifest,[Parameter(Mandatory=$true)][string]$Destination)
$ErrorActionPreference='Stop'
if(Test-Path -LiteralPath $Destination){throw 'Evidence destination must be new'}
$manifestData=[IO.File]::ReadAllText((Resolve-Path -LiteralPath $Manifest).Path) | ConvertFrom-Json
if($manifestData.computer -ne 'DESKTOP-24NJTT6' -or !$manifestData.files){throw 'Wrong manifest'}
$sources=@(foreach($file in $manifestData.files){
 if($file.name -notmatch '^(real-(scan|running-guard|cleanup)-[a-f0-9-]+\.json|reopen-dingtalk-(http|code)\.json|real-test-binary\.json|ordinary-http-protection-(before|after)\.json|ordinary-(home-build|home-launch|protection-build)\.json|(DingTalk|BaiduNetdisk)\.install-preflight\.json)$'){throw 'Unexpected evidence file'}
 'MSN@100.69.113.49:C:/Users/MSN/AppData/Local/DustAwayAcceptance/20261001-home-cleanup/'+$file.name
})
$target=(New-Item -ItemType Directory -Path $Destination).FullName.Replace('\','/')
$posixTarget='/'+$target.Substring(0,1).ToLower()+'/'+$target.Substring(3)+'/'
$copyArgs=@('-F','/dev/null','-i',(Join-Path $env:USERPROFILE '.ssh/dustaway-desktop-24njtt6-ed25519').Replace('\','/'),'-o','IdentitiesOnly=yes','-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o',('UserKnownHostsFile='+(Join-Path $env:USERPROFILE '.ssh/dustaway-desktop-24njtt6-known-hosts').Replace('\','/')),'-o','GlobalKnownHostsFile=/dev/null','-o','HostKeyAlgorithms=ssh-ed25519')
& 'C:\Program Files\Git\usr\bin\scp.exe' @copyArgs @sources $posixTarget
if($LASTEXITCODE -ne 0){throw 'Evidence transfer incomplete'}
foreach($file in $manifestData.files){
 $copied=Join-Path $target $file.name
 if((Get-Item -LiteralPath $copied).Length -ne $file.bytes -or (Get-FileHash -LiteralPath $copied -Algorithm SHA256).Hash -ne $file.sha256){throw ('Evidence hash mismatch: '+$file.name)}
}
@{files=$sources.Count;hashes_verified=$true;source='dedicated-remote-test-directory'} | ConvertTo-Json
