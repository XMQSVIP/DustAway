param([Parameter(Mandatory=$true)][ValidateSet('qq_classic','qqnt')][string]$Provider)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$taskRoot = Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261002-qq'
if ($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $PSScriptRoot -ne $taskRoot) { throw 'Dedicated test computer and directory required' }
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Ordinary user required' }
$binary = Join-Path $taskRoot 'attachments_probe.exe'
$expected = (Get-Content -Raw -LiteralPath (Join-Path $taskRoot 'probe-binary.json') | ConvertFrom-Json).sha256
if ((Get-FileHash -LiteralPath $binary).Hash -ne $expected) { throw 'Unexpected probe binary' }
$previous = $env:DUSTAWAY_ATTACHMENT_PROVIDER
try {
  $env:DUSTAWAY_ATTACHMENT_PROVIDER = $Provider
  & $binary *> (Join-Path $taskRoot ($Provider + '-probe.json'))
  $code = $LASTEXITCODE
  @{provider=$Provider; exit_code=$code; binary_sha256=$expected; deletion_attempted=$false; time=(Get-Date -Format o)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot ($Provider + '-probe-result.json')) -Encoding UTF8
  exit $code
} finally {
  $env:DUSTAWAY_ATTACHMENT_PROVIDER = $previous
}
