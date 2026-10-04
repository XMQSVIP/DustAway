$ErrorActionPreference='Stop'
if($env:COMPUTERNAME-ne'DESKTOP-24NJTT6'){throw 'Wrong host'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20261002-rule-refresh-v4'
$result=@{root_exists=(Test-Path -LiteralPath $taskRoot);processes=@(Get-Process -Name dustaway,tauri-driver,msedgedriver,chrome,node,YunDetectService -ErrorAction SilentlyContinue | Select-Object ProcessName,Id);tasks=@(Get-ScheduledTask | Where-Object {$_.TaskName-like'DustAway-RuleRefresh-*'} | ForEach-Object {@{name=$_.TaskName;state=[string]$_.State;last=(Get-ScheduledTaskInfo -TaskName $_.TaskName).LastTaskResult}})}
if(Test-Path -LiteralPath (Join-Path $taskRoot 'evidence')){$result.evidence=@(Get-ChildItem -LiteralPath (Join-Path $taskRoot 'evidence') -Recurse -File | Where-Object {$_.Name -in @('run.json','error.txt','stderr.log','summary.json','desktop-results.json')} | ForEach-Object {@{path=$_.FullName.Substring($taskRoot.Length+1);text=$(try {[IO.File]::ReadAllText($_.FullName)} catch {"Still being written"})}})}
$taskPerf=Join-Path $taskRoot 'evidence/performance'
$result.matrix_files=@(Get-ChildItem -LiteralPath (Join-Path $taskRoot 'evidence/matrix') -File -ErrorAction SilentlyContinue | Select-Object Name,Length,LastWriteTime)
$result.real_files=@(Get-ChildItem -LiteralPath (Join-Path $taskRoot 'evidence/real') -File -ErrorAction SilentlyContinue | Select-Object Name,Length,LastWriteTime)
$result.chrome_files=@(Get-ChildItem -LiteralPath (Join-Path $taskRoot 'evidence/chrome') -Recurse -File -ErrorAction SilentlyContinue | Where-Object {$_.Extension-eq'.json'} | ForEach-Object {@{path=$_.FullName.Substring($taskRoot.Length+1);bytes=$_.Length}})
if(Test-Path -LiteralPath $taskPerf){$result.performance=@('run.json','stdout.log','stderr.log','comparison/summary.json')|ForEach-Object{$p=Join-Path $taskPerf $_;if(Test-Path -LiteralPath $p){try{$s=[IO.File]::Open($p,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::ReadWrite);$r=New-Object IO.StreamReader($s);try{$t=$r.ReadToEnd()}finally{$r.Dispose()};@{path=$_;text=$t}}catch{@{path=$_;text='Still being written'}}}}}
$result | ConvertTo-Json -Depth 6
