param([Parameter(Mandatory=$true)][ValidateSet('Running','Delete')][string]$Phase)
$ErrorActionPreference = 'Stop'
$taskRoot = Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261002-attachments'
if ($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $PSScriptRoot -ne $taskRoot) { throw 'Wrong environment' }
if ([IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-test-only')) -ne 'DustAway.Attachments.20261002') { throw 'Wrong marker' }
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Ordinary user required' }
$binary = Join-Path $taskRoot 'attachments-tests.exe'
$expected = (Get-Content -Raw -LiteralPath (Join-Path $taskRoot 'attachments-test-binary.json') | ConvertFrom-Json).sha256
if ((Get-FileHash -LiteralPath $binary).Hash -ne $expected) { throw 'Unexpected test binary' }
$test = if ($Phase -eq 'Running') {'attachments::acceptance::real_running_guard'} else {'attachments::acceptance::real_delete_once'}
if (Test-Path -LiteralPath (Join-Path $taskRoot ($Phase + '.log'))) {
  Copy-Item -LiteralPath (Join-Path $taskRoot ($Phase + '.log')) -Destination (Join-Path $taskRoot ($Phase + '-' + (Get-Date -Format 'HHmmss') + '.previous.log'))
}
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$ErrorActionPreference = 'Continue'
& $binary '--exact' $test '--ignored' '--nocapture' '--test-threads=1' *> (Join-Path $taskRoot ($Phase + '.log'))
$code = $LASTEXITCODE
$ErrorActionPreference = 'Stop'
@{phase=$Phase; exit_code=$code; sha256=$expected; time=(Get-Date -Format o)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot ($Phase + '.result.json')) -Encoding UTF8
exit $code
