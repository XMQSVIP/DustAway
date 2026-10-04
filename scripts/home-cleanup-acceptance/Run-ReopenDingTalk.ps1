$ErrorActionPreference='Stop'
$root=$PSScriptRoot
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $root -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup')){throw 'Wrong environment'}
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
$principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
$receipts=@(Get-ChildItem -LiteralPath $root -Filter 'real-cleanup-*.json' | ForEach-Object {[IO.File]::ReadAllText($_.FullName) | ConvertFrom-Json} | Where-Object {$_.rule_id -eq 'dingtalk.http'})
if($receipts.Count -ne 1 -or !$receipts[0].protection_unchanged -or $receipts[0].report.deleted -ne 8 -or $receipts[0].report.failed -ne 0 -or $receipts[0].report.unknown -ne 0){throw 'Cleanup result requires review before relaunch'}
if(@(Get-Process | Where-Object {$_.ProcessName -match '^DingTalk'}).Count){throw 'Application already running; do not repeat'}
$exe='C:\Program Files (x86)\DingDing\main\current\DingTalk.exe'
if((Get-FileHash -LiteralPath $exe).Hash -ne '5705DE940C8BF048792AF1E99B55C19F914FC1CA89B6DDB7AB3C4AC0595F69F4' -or (Get-AuthenticodeSignature -LiteralPath $exe).Status -ne 'Valid'){throw 'Program identity changed'}
$once=[IO.File]::Open((Join-Path $root 'reopen-dingtalk-http.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None);$once.Dispose()
$process=Start-Process -FilePath $exe -WorkingDirectory (Split-Path -Parent $exe) -PassThru
@{time=(Get-Date -Format o);pid=$process.Id;stage='reopened_after_http_cleanup';functional_check='pending';automatic_login_attempted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'reopen-dingtalk-http.json') -Encoding UTF8
