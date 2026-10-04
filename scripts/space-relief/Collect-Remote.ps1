$ErrorActionPreference='Stop'
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20261002-space-relief'
if($env:COMPUTERNAME-ne'DESKTOP-24NJTT6'-or[IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-space-relief'))-ne'DustAway.SpaceRelief.20261002'){throw 'Wrong host/root'}
if(Get-ScheduledTask | Where-Object {$_.TaskName-like'DustAway-SpaceRelief-*'-and$_.State-eq'Running'}){throw 'Still running'}
$taskZip=Join-Path $taskRoot 'evidence.zip'
if(Test-Path -LiteralPath $taskZip){throw 'Already collected'}
$taskFiles=@(Get-ChildItem -LiteralPath (Join-Path $taskRoot 'evidence'),(Join-Path $taskRoot 'scripts') -Recurse -File)
$taskFiles+=Get-Item -LiteralPath (Join-Path $taskRoot 'build.json')
@($taskFiles | ForEach-Object {@{path=$_.FullName.Substring($taskRoot.Length+1);sha256=(Get-FileHash -LiteralPath $_.FullName).Hash;bytes=$_.Length}}) | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $taskRoot 'evidence-manifest.json') -Encoding UTF8
Compress-Archive -Path (Join-Path $taskRoot 'evidence'),(Join-Path $taskRoot 'scripts'),(Join-Path $taskRoot 'build.json'),(Join-Path $taskRoot 'evidence-manifest.json') -DestinationPath $taskZip
@{sha256=(Get-FileHash -LiteralPath $taskZip).Hash;path=$taskZip} | ConvertTo-Json
