$ErrorActionPreference='Stop'
$root=$PSScriptRoot
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $root -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup')){throw 'Wrong environment'}
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
$principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
$receipts=@(Get-ChildItem -LiteralPath $root -Filter 'real-cleanup-*.json' | ForEach-Object {[IO.File]::ReadAllText($_.FullName) | ConvertFrom-Json} | Where-Object {$_.rule_id -eq 'dingtalk.code'})
if($receipts.Count -ne 1){throw 'Expected one code-cache receipt'}
$receipt=$receipts[0]
if(!$receipt.execution_returned_ok -or !$receipt.protection_unchanged -or $receipt.version -ne '8.2.0.119' -or $receipt.report.status -ne 'finished' -or $receipt.report.deleted -le 0 -or $receipt.report.failed -ne 0 -or $receipt.report.skipped -ne 0 -or $receipt.report.unknown -ne 0){throw 'Cleanup result requires review before relaunch'}
if(@(Get-Process | Where-Object {$_.ProcessName -match '^DingTalk'}).Count){throw 'Application already running; do not repeat'}
$exe='C:\Program Files (x86)\DingDing\main\current\DingTalk.exe'
if((Get-FileHash -LiteralPath $exe).Hash -ne '5705DE940C8BF048792AF1E99B55C19F914FC1CA89B6DDB7AB3C4AC0595F69F4' -or (Get-AuthenticodeSignature -LiteralPath $exe).Status -ne 'Valid'){throw 'Program identity changed'}
$once=[IO.File]::Open((Join-Path $root 'reopen-dingtalk-code.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
try {$bytes=[Text.Encoding]::UTF8.GetBytes($receipt.report.id);$once.Write($bytes,0,$bytes.Length);$once.Flush($true)} finally {$once.Dispose()}
$process=Start-Process -FilePath $exe -WorkingDirectory (Split-Path -Parent $exe) -PassThru
@{time=(Get-Date -Format o);pid=$process.Id;cleanup_plan=$receipt.report.id;stage='reopened_after_code_cleanup';functional_check='pending';automatic_login_attempted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'reopen-dingtalk-code.json') -Encoding UTF8
