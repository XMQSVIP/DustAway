$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-developer-cleanup'
if([IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-developer')) -ne 'DustAway.Developer.20261001'){throw 'Wrong root'}
if((Get-ScheduledTask -TaskName 'DustAway-Developer-20261001').State -eq 'Running' -or (Get-ScheduledTask -TaskName 'DustAway-Developer-DesktopV2-20261001').State -eq 'Running'){throw 'Test still running'}
$taskFiles=@(Get-ChildItem -LiteralPath (Join-Path $taskRoot 'evidence'),(Join-Path $taskRoot 'evidence-desktop-v2') -File)
$taskFiles+=@(Get-ChildItem -LiteralPath (Join-Path $taskRoot 'scripts/developer-cleanup') -File)
$taskFiles+=@(Get-Item -LiteralPath (Join-Path $taskRoot 'build.json'))
$taskManifest=@($taskFiles | ForEach-Object {[ordered]@{path=$_.FullName.Substring($taskRoot.Length+1);bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}})
$taskManifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskRoot 'evidence-manifest.json') -Encoding UTF8
$taskZip=Join-Path $taskRoot 'evidence.zip'
if(Test-Path -LiteralPath $taskZip){throw 'Already collected'}
Compress-Archive -Path (Join-Path $taskRoot 'evidence'),(Join-Path $taskRoot 'evidence-desktop-v2'),(Join-Path $taskRoot 'scripts'),(Join-Path $taskRoot 'build.json'),(Join-Path $taskRoot 'evidence-manifest.json') -DestinationPath $taskZip
[ordered]@{sha256=(Get-FileHash -LiteralPath $taskZip).Hash;files=$taskFiles.Count;processes=@(Get-Process -Name dustaway,tauri-driver,msedgedriver -ErrorAction SilentlyContinue | Select-Object ProcessName,Id)} | ConvertTo-Json -Depth 3
