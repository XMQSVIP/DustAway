param([Parameter(Mandatory=$true)][ValidateSet('dingtalk.http','dingtalk.code','baidu_netdisk.http','baidu_netdisk.code','baidu_netdisk.gpu')][string]$Rule)
$ErrorActionPreference='Stop'
$root=$PSScriptRoot
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $root -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup')){throw 'Wrong environment'}
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong marker'}
$principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
if(Test-Path -LiteralPath (Join-Path $root ('real-clean-'+$Rule+'.started'))){throw 'Already executed or pending; never replay cleanup'}
$binary=Join-Path $root 'dustaway-real-app-tests.exe'
$manifest=Get-Content -LiteralPath (Join-Path $root 'real-test-binary.json') -Raw | ConvertFrom-Json
if(!$manifest.deletion_entry_present -or (Get-FileHash -LiteralPath $binary).Hash -ne $manifest.sha256){throw 'Unverified test binary'}
$run=Get-Date -Format 'yyyyMMdd-HHmmss'
$env:DUSTAWAY_HOME_RULE=$Rule
$ErrorActionPreference='Continue'
& $binary '--exact' 'platform::real_application_acceptance::real_application_cleanup' '--ignored' '--nocapture' '--test-threads=1' *> (Join-Path $root ('cleanup-'+$Rule+'-'+$run+'.log'))
$result=$LASTEXITCODE
$ErrorActionPreference='Stop'
@{time=(Get-Date -Format o);rule=$Rule;binary_sha256=$manifest.sha256;exit_code=$result;relaunch_functional_check='not_run';release_approved=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root ('cleanup-'+$Rule+'-'+$run+'.json')) -Encoding UTF8
exit $result
