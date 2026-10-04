param([Parameter(Mandatory=$true)][ValidateSet('Before','After')][string]$Phase)
$ErrorActionPreference='Stop'
$root=$PSScriptRoot
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $root -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup')){throw 'Wrong environment'}
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
$principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
$binary=Join-Path $root 'dustaway-ordinary-protection-tests.exe'
$manifest=[IO.File]::ReadAllText((Join-Path $root 'ordinary-protection-build.json')) | ConvertFrom-Json
if($manifest.kind -ne 'ordinary-protection-checkpoint-test' -or (Get-FileHash -LiteralPath $binary).Hash -ne $manifest.sha256){throw 'Checkpoint binary identity mismatch'}
$env:DUSTAWAY_HOME_PROTECTION_PHASE=$Phase.ToLowerInvariant()
$log=Join-Path $root ('ordinary-protection-'+$Phase.ToLowerInvariant()+'-'+(Get-Date -Format 'yyyyMMdd-HHmmss')+'.log')
if(Test-Path -LiteralPath $log){throw 'Log already exists'}
try {
 $ErrorActionPreference='Continue'
 & $binary '--exact' 'platform::real_application_acceptance::ordinary_home_protection' '--ignored' '--nocapture' '--test-threads=1' *> $log
 $result=$LASTEXITCODE
} finally {Remove-Item Env:DUSTAWAY_HOME_PROTECTION_PHASE}
exit $result
