param([Parameter(Mandatory=$true)][ValidateSet('Running','DeleteOnce')][string]$Phase)
$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[Text.Encoding]::UTF8
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261002-qq'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $PSScriptRoot -ne $taskRoot){throw 'Dedicated test computer and directory required'}
$principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
$binary=Join-Path $taskRoot 'qqnt-acceptance-tests.exe'
$expected=(Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $taskRoot 'nt-test-binary.json') | ConvertFrom-Json).sha256
if((Get-FileHash -LiteralPath $binary).Hash -ne $expected){throw 'Unexpected acceptance binary'}
if($Phase -eq 'DeleteOnce' -and (Test-Path -LiteralPath (Join-Path $taskRoot 'nt-delete.started'))){throw 'Never replay this deletion'}
$test=if($Phase -eq 'Running'){'qqnt_running_guard'}else{'qqnt_delete_once'}
$process=Start-Process -FilePath $binary -ArgumentList @(('attachments::qq::nt_acceptance::'+$test),'--ignored','--exact','--nocapture','--test-threads=1') -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot ('nt-'+$Phase+'-test.log')) -RedirectStandardError (Join-Path $taskRoot ('nt-'+$Phase+'-stderr.log'))
$code=$process.ExitCode
@{phase=$Phase;exit_code=$code;binary_sha256=$expected;time=(Get-Date -Format o)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot ('nt-'+$Phase+'-result.json')) -Encoding UTF8
exit $code
