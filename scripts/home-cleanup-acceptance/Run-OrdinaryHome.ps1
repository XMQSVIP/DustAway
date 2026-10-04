$ErrorActionPreference='Stop'
$root=$PSScriptRoot
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $root -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup')){throw 'Wrong environment'}
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
$principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
if(@(Get-CimInstance Win32_Process | Where-Object {$_.Name -like '*dustaway*.exe' -or $_.Name -eq '拂尘.exe'}).Count){throw 'DustAway already running; inspect before proceeding'}
$before=[IO.File]::ReadAllText((Join-Path $root 'ordinary-http-protection-before.json')) | ConvertFrom-Json
if($before.phase -ne 'before' -or $before.rule_id -ne 'dingtalk.http' -or $before.version -ne '8.2.0.119' -or $before.deletion_attempted -ne $false -or $before.protected_file_count -le 0 -or (Test-Path -LiteralPath (Join-Path $root 'ordinary-http-protection-after.json'))){throw 'A fresh protection baseline is required'}
$manifest=[IO.File]::ReadAllText((Join-Path $root 'ordinary-home-build.json')) | ConvertFrom-Json
$exe=Join-Path $root 'dustaway-home-cleanup.exe'
if($manifest.kind -ne 'ordinary-debug-custom-protocol' -or $manifest.e2e_fixture -ne $false -or $manifest.test_permit -ne $false -or $manifest.accepted_rule -ne 'dingtalk.http' -or $manifest.accepted_version -ne '8.2.0.119' -or (Get-FileHash -LiteralPath $exe).Hash -ne $manifest.sha256){throw 'Ordinary build identity mismatch'}
# This opens the ordinary UI only. It never scans, selects, confirms or deletes.
$once=[IO.File]::Open((Join-Path $root 'ordinary-home.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
try {$bytes=[Text.Encoding]::UTF8.GetBytes($manifest.sha256);$once.Write($bytes,0,$bytes.Length);$once.Flush($true)} finally {$once.Dispose()}
$process=Start-Process -FilePath $exe -WorkingDirectory $root -WindowStyle Normal -PassThru
@{time=(Get-Date -Format o);pid=$process.Id;stage='ordinary_ui_opened';sha256=$manifest.sha256;automatic_scan=$false;automatic_deletion=$false;homepage_acceptance='pending'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'ordinary-home-launch.json') -Encoding UTF8
