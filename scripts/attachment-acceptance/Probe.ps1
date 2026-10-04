$ErrorActionPreference = 'Stop'
$taskRoot = Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261002-attachments'
if ($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $PSScriptRoot -ne $taskRoot) { throw 'Wrong environment' }
if ([IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-test-only')) -ne 'DustAway.Attachments.20261002') { throw 'Wrong marker' }
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Ordinary user required' }
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$binary = Join-Path $taskRoot 'attachments_probe.exe'
& $binary > (Join-Path $taskRoot 'final-readonly-scan.json') 2> (Join-Path $taskRoot 'final-readonly-scan-errors.log')
$code = $LASTEXITCODE
@{exit_code=$code;sha256=(Get-FileHash -LiteralPath $binary).Hash;time=(Get-Date -Format o)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'final-readonly-result.json') -Encoding UTF8
exit $code
