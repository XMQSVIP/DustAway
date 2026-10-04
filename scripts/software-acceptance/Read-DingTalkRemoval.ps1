$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
$out=Join-Path $root 'dingtalk-removal-20261001'
$evidence=@{}
foreach($name in @('status.json','session.json','scan-summary.json')){
 $file=Join-Path $out $name
 if(Test-Path -LiteralPath $file){$evidence[$name]=[IO.File]::ReadAllText($file) | ConvertFrom-Json}
}
foreach($name in @('status.json','session.json','scan-summary.json')){
 $file=Join-Path (Join-Path $root 'dingtalk-removal-path-fix') $name
 if(Test-Path -LiteralPath $file){$evidence['path-fix-'+$name]=[IO.File]::ReadAllText($file) | ConvertFrom-Json}
}
@{time=(Get-Date -Format o);evidence=$evidence;error_log=if(Test-Path -LiteralPath (Join-Path $root 'dingtalk-removal-error.log')){Get-Content -LiteralPath (Join-Path $root 'dingtalk-removal-error.log') -Tail 10};processes=@(Get-CimInstance Win32_Process | Where-Object {$_.Name -match 'DingTalk|DingDing|dustaway|tauri-driver|msedgedriver' -or $_.ExecutablePath -like '*\DingDing\*' -or $_.Name -match '^un.*\.exe$'} | Select-Object Name,ProcessId,SessionId,ExecutablePath);task=Get-ScheduledTask -TaskName 'DustAway-DingTalk-Removal-20261001' | Select-Object TaskName,State} | ConvertTo-Json -Depth 10
