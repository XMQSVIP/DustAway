param([Parameter(Mandatory=$true)][ValidateSet('Scan','BaiduRunning','DingTalkRunning')][string]$Probe)
$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $PSScriptRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup')){throw 'Wrong environment'}
$root=$PSScriptRoot
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong marker'}
$principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
$binary=Join-Path $root 'dustaway-real-app-tests.exe'
$expected=(Get-Content -LiteralPath (Join-Path $root 'real-test-binary.json') -Raw | ConvertFrom-Json).sha256
if((Get-FileHash -LiteralPath $binary).Hash -ne $expected){throw 'Test binary changed'}
$run=Get-Date -Format 'yyyyMMdd-HHmmss'
$receipts=@()
if($Probe -eq 'Scan'){
 $ErrorActionPreference='Continue'
 & $binary '--exact' 'platform::real_application_acceptance::real_application_scan' '--ignored' '--nocapture' '--test-threads=1' *> (Join-Path $root ('probe-scan-'+$run+'.log'))
 $receipts+=@{probe='scan';exit_code=$LASTEXITCODE}
 $ErrorActionPreference='Stop'
}else{
 $ids=if($Probe -eq 'DingTalkRunning'){@('dingtalk.http','dingtalk.code')}else{@('baidu_netdisk.http','baidu_netdisk.code','baidu_netdisk.gpu')}
 foreach($id in $ids){
  $env:DUSTAWAY_HOME_RULE=$id
  $ErrorActionPreference='Continue'
  & $binary '--exact' 'platform::real_application_acceptance::real_application_running_guard' '--ignored' '--nocapture' '--test-threads=1' *> (Join-Path $root ('probe-'+$id+'-'+$run+'.log'))
  $receipts+=@{rule=$id;exit_code=$LASTEXITCODE}
  $ErrorActionPreference='Stop'
 }
}
@{time=(Get-Date -Format o);probe=$Probe;results=$receipts;deletion_attempted=$false} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root ('probe-'+$Probe+'-'+$run+'.json')) -Encoding UTF8
if(@($receipts | Where-Object {$_.exit_code -ne 0}).Count){exit 1}
