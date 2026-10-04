$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-developer-mainstream-v2'
if([IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-developer')) -ne 'DustAway.Mainstream.20261001.V2'){throw 'Wrong root'}
if((Get-ScheduledTask -TaskName 'DustAway-Mainstream-20261001-v2').State -eq 'Running'){throw 'Test still running'}
$taskProtected=@((Get-Content -LiteralPath (Join-Path $taskRoot 'evidence/fixtures.json') -Raw | ConvertFrom-Json).protected | ForEach-Object {
 $taskObserved=(Get-FileHash -LiteralPath $_.path).Hash
 if($taskObserved -ne $_.sha256){throw 'Protected fixture changed'}
 [ordered]@{path=$_.path;sha256=$taskObserved;expected=$_.sha256;unchanged=$true}
})
$taskProtected | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskRoot 'evidence/protected-final.json') -Encoding UTF8
$taskFiles=@(Get-ChildItem -LiteralPath (Join-Path $taskRoot 'evidence') -File)
$taskFiles+=@(Get-ChildItem -LiteralPath (Join-Path $taskRoot 'scripts/developer-mainstream-v2') -File)
$taskFiles+=@(Get-Item -LiteralPath (Join-Path $taskRoot 'build.json'))
$taskManifest=@($taskFiles | ForEach-Object {[ordered]@{path=$_.FullName.Substring($taskRoot.Length+1);bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}})
$taskManifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskRoot 'evidence-manifest.json') -Encoding UTF8
$taskZip=Join-Path $taskRoot 'evidence.zip'
if(Test-Path -LiteralPath $taskZip){throw 'Already collected'}
Compress-Archive -Path (Join-Path $taskRoot 'evidence'),(Join-Path $taskRoot 'scripts'),(Join-Path $taskRoot 'build.json'),(Join-Path $taskRoot 'evidence-manifest.json') -DestinationPath $taskZip
[ordered]@{sha256=(Get-FileHash -LiteralPath $taskZip).Hash;files=$taskFiles.Count;processes=@(Get-Process -Name dustaway,tauri-driver,msedgedriver -ErrorAction SilentlyContinue | Select-Object ProcessName,Id)} | ConvertTo-Json -Depth 3
