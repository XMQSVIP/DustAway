$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong test root'}
$files=@(Get-ChildItem -LiteralPath $root -File | Where-Object {$_.Name -match '^(real-(scan|running-guard|cleanup)-[a-f0-9-]+\.json|reopen-dingtalk-(http|code)\.json|real-test-binary\.json|ordinary-http-protection-(before|after)\.json|ordinary-(home-build|home-launch|protection-build)\.json|(DingTalk|BaiduNetdisk)\.install-preflight\.json)$'} | Sort-Object Name | ForEach-Object {@{name=$_.Name;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash}})
@{time=(Get-Date -Format o);computer=$env:COMPUTERNAME;files=$files} | ConvertTo-Json -Depth 4
