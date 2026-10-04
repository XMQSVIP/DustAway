$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-cleanup-completion'
Get-ScheduledTask | Where-Object {$_.TaskName -like 'DustAway-CleanupCompletion-*'} | Select-Object TaskName,State | ConvertTo-Json -Compress
Get-ChildItem -LiteralPath (Join-Path $taskRoot 'evidence/chrome-native-receipt') -Recurse -File -ErrorAction SilentlyContinue | Select-Object Name,Length | ConvertTo-Json -Compress
Get-ChildItem -LiteralPath (Join-Path $taskRoot 'evidence') -Recurse -File -ErrorAction SilentlyContinue | Where-Object {$_.Name -in @('result.json','error.txt','results.json','chrome-reopen.json') -or $_.Name -like '*.error.log'} | ForEach-Object {
 if($_.Length -gt 0 -and $_.FullName -notmatch 'chrome-first'){$_.FullName; [IO.File]::ReadAllText($_.FullName) | Select-Object -Last 1}
}
