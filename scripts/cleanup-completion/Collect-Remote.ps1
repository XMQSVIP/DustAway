$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-cleanup-completion'
if([IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-completion')) -ne 'DustAway.CleanupCompletion.20261001'){throw 'Wrong root'}
if(@(Get-ScheduledTask | Where-Object {$_.TaskName -like 'DustAway-CleanupCompletion-*' -and $_.State -eq 'Running'}).Count){throw 'Tests are still running'}
$taskZip=Join-Path $taskRoot 'cleanup-completion-evidence.zip'
if(Test-Path -LiteralPath $taskZip){throw 'Evidence already collected'}
$taskFiles=@(Get-ChildItem -LiteralPath (Join-Path $taskRoot 'evidence'),(Join-Path $taskRoot 'scripts') -Recurse -File)
$taskFiles+=@(Get-Item -LiteralPath (Join-Path $taskRoot 'build.json'))
$taskFiles+=@(Get-ChildItem -LiteralPath $taskRoot -Filter '*.cleanup-started' -File)
$taskManifest=@($taskFiles | ForEach-Object {[ordered]@{path=$_.FullName.Substring($taskRoot.Length+1);bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}})
$taskManifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskRoot 'evidence-manifest.json') -Encoding UTF8
Compress-Archive -Path (Join-Path $taskRoot 'evidence'),(Join-Path $taskRoot 'scripts'),(Join-Path $taskRoot 'build.json'),(Join-Path $taskRoot 'evidence-manifest.json'),(Join-Path $taskRoot '*.cleanup-started') -DestinationPath $taskZip
[ordered]@{file=$taskZip;files=$taskFiles.Count;bytes=(Get-Item -LiteralPath $taskZip).Length;sha256=(Get-FileHash -LiteralPath $taskZip).Hash;processes=@(Get-Process -Name dustaway,tauri-driver,msedgedriver,chrome -ErrorAction SilentlyContinue | Select-Object ProcessName,Id)} | ConvertTo-Json -Depth 4
