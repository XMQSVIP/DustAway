$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
$names=@('dingtalk-removal-20261001/status.json','dingtalk-removal-20261001/session.json','dingtalk-removal-path-fix/status.json','dingtalk-removal-path-fix/session.json','dingtalk-removal-path-fix/target.json','dingtalk-removal-path-fix/scan-summary.json','dingtalk-removal-path-fix/uninstall-submit.started','dingtalk-path-build.json','dingtalk-removal-finished.json')
$files=@(foreach($name in $names){$path=Join-Path $root $name;@{name=$name;bytes=(Get-Item -LiteralPath $path).Length;sha256=(Get-FileHash -LiteralPath $path).Hash}})
$tasks=@(foreach($name in @('DustAway-DingTalk-Removal-20261001','DustAway-DingTalk-Removal-PathFix-20261001')){Get-ScheduledTask -TaskName $name | Select-Object TaskName,State})
@{computer=$env:COMPUTERNAME;time=(Get-Date -Format o);files=$files;tasks=$tasks;processes=@(Get-CimInstance Win32_Process | Where-Object {$_.Name -match 'DingTalk|DingDing|dustaway|tauri-driver|msedgedriver'} | Select-Object Name,ProcessId,ExecutablePath)} | ConvertTo-Json -Depth 6
