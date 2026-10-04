$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
$names=@('dustaway-home-cleanup.exe','ordinary-home-build.json','ordinary-home.started','ordinary-home-launch.json','dustaway-ordinary-protection-tests.exe','ordinary-protection-build.json','ordinary-http-protection-before.json','ordinary-http-protection-after.json')
$files=@(foreach($name in $names){$path=Join-Path $root $name; if(Test-Path -LiteralPath $path){$item=Get-Item -LiteralPath $path; @{name=$name;bytes=$item.Length;sha256=(Get-FileHash -LiteralPath $path).Hash}}})
$active=@(Get-ScheduledTask | Where-Object {$_.TaskName -like 'DustAway-HomeCleanup-*' -and $_.State -eq 'Running'} | Select-Object TaskName,State)
$processes=@(Get-CimInstance Win32_Process | Where-Object {$_.Name -match 'DingTalk|BaiduNetdisk|YunDetect|YunUtility|dustaway' -or $_.Name -eq '拂尘.exe'} | Select-Object Name,ProcessId,SessionId,ExecutablePath)
@{time=(Get-Date -Format o);computer=$env:COMPUTERNAME;files=$files;active_tasks=$active;processes=$processes;deletion_attempted=$false} | ConvertTo-Json -Depth 5
