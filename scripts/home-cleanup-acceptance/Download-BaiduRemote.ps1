$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
$target=Join-Path $root 'BaiduNetdisk.exe'
if(Test-Path -LiteralPath $target){throw 'Download already attempted; inspect existing file'}
$manifest=Get-Content -LiteralPath (Join-Path $root 'BaiduNetdisk.manifest.json') -Raw | ConvertFrom-Json
& "$env:SystemRoot\System32\curl.exe" --noproxy '*' --fail --location --silent --show-error --connect-timeout 15 --max-time 180 'https://issuepcdn.baidupcs.com/issue/netdisk/yunguanjia/x64/BaiduNetdisk_8.8.8.101_x64.exe' -o $target
if($LASTEXITCODE -ne 0){throw 'Direct download failed; partial file retained, do not execute'}
if((Get-FileHash -LiteralPath $target).Hash -ne $manifest.sha256){throw 'Downloaded bytes differ from verified local installer'}
[ordered]@{time=(Get-Date -Format o);downloaded=$true;sha256=$manifest.sha256;proxy='disabled for curl process only'} | ConvertTo-Json
