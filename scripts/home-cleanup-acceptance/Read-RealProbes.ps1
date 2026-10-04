$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
$files=@(Get-ChildItem -LiteralPath $root -File | Where-Object {$_.Name -match '^(cleanup-|probe-|real-(cleanup|scan|running-guard)-)' -and $_.Extension -in @('.json','.log')} | Sort-Object LastWriteTime)
$outputs=@(foreach($file in $files){try{@{name=$file.Name;content=[IO.File]::ReadAllText($file.FullName)}}catch{@{name=$file.Name;content=$null;read_error=$_.Exception.GetType().Name}}})
$tasks=@(Get-ScheduledTask | Where-Object {$_.TaskName -like 'DustAway-HomeCleanup-RealProbe-*' -or $_.TaskName -like 'DustAway-HomeCleanup-Clean-*'} | ForEach-Object {@{name=$_.TaskName;state=[string]$_.State;result=($_ | Get-ScheduledTaskInfo).LastTaskResult}})
@{time=(Get-Date -Format o);tasks=$tasks;outputs=$outputs} | ConvertTo-Json -Depth 5
