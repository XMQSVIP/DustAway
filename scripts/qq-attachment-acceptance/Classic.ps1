param([Parameter(Mandatory=$true)][ValidateSet('Running','DeleteOnce','SpaceDeleteOnce')][string]$Phase)
$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[Text.Encoding]::UTF8
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261002-qq'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $PSScriptRoot -ne $taskRoot){throw 'Dedicated test computer and directory required'}
$principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
$binary=Join-Path $taskRoot 'qq-acceptance-tests.exe'
$expected=(Get-Content -Raw -LiteralPath (Join-Path $taskRoot 'classic-test-binary.json') | ConvertFrom-Json).sha256
if((Get-FileHash -LiteralPath $binary).Hash -ne $expected){throw 'Unexpected acceptance binary'}
if($Phase -eq 'DeleteOnce' -and (Test-Path -LiteralPath (Join-Path $taskRoot 'classic-delete.started'))){throw 'Never replay this deletion'}
if($Phase -eq 'SpaceDeleteOnce' -and (Test-Path -LiteralPath (Join-Path $taskRoot 'classic-space-delete.started'))){throw 'Never replay this deletion'}
$test=switch($Phase){'Running'{'classic_running_guard'} 'DeleteOnce'{'classic_delete_once'} 'SpaceDeleteOnce'{'classic_space_delete_once'}}
$process=Start-Process -FilePath $binary -ArgumentList @(('attachments::qq::acceptance::'+$test),'--ignored','--exact','--nocapture','--test-threads=1') -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot ('classic-'+$Phase+'-test.log')) -RedirectStandardError (Join-Path $taskRoot ('classic-'+$Phase+'-stderr.log'))
$code=$process.ExitCode
@{phase=$Phase;exit_code=$code;binary_sha256=$expected;time=(Get-Date -Format o)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot ('classic-'+$Phase+'-result.json')) -Encoding UTF8
exit $code
