$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
$folder=Join-Path $root 'dingtalk-removal-path-fix'
$session=[IO.File]::ReadAllText((Join-Path $folder 'session.json')) | ConvertFrom-Json
$summary=[IO.File]::ReadAllText((Join-Path $folder 'scan-summary.json')) | ConvertFrom-Json
if($summary.task.id -ne 'b8081baa-afde-48d8-9bd9-f47f938ef54e' -or $summary.task.uninstall_status -ne 'removed' -or $summary.deletion_executed -ne $false){throw 'Unexpected task state'}
if(@(Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -like '*\DingDing\*'}).Count){throw 'Vendor process still running'}
$reply=Invoke-RestMethod -Method Delete -Uri ('http://127.0.0.1:4444/session/'+$session.session) -TimeoutSec 15
$old=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software\firefox-desktop-delete-20261001-v2'
# Only the two automation helpers started by this operation, after closing its session.
foreach($owned in @(@{id=7660;path=(Join-Path $old 'bin\tauri-driver.exe')},@{id=6844;path=(Join-Path $old 'artifacts\webdriver\msedgedriver.exe')})){
 $process=Get-CimInstance Win32_Process -Filter ('ProcessId='+$owned.id)
 if($process){if($process.ExecutablePath -ne $owned.path -or $process.SessionId -ne 2){throw 'Helper identity changed'};Stop-Process -Id $owned.id}
}
$result=@{time=(Get-Date -Format o);uninstall_status='removed';residual_scan_complete=$false;residual_deleted_bytes=0;reason='Pre-uninstall ownership capture unavailable';ordinary_ui_closed=$true;production_permission_added=$false;active_vendor_processes=0}
$result | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'dingtalk-removal-finished.json') -Encoding UTF8
$result | ConvertTo-Json
