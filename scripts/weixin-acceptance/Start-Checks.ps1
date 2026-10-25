param([ValidatePattern('^[A-Za-z0-9-]{8,80}$')][string]$RunId = ('readonly-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss')))
$ErrorActionPreference='Stop'
$root='C:\Users\MSN\AppData\Local\DustAwayAcceptance\weixin-021'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or [IO.File]::ReadAllText((Join-Path $root '.dustaway-test')) -cne 'DustAway.Weixin.0.21'){throw 'Wrong host/marker'}
$task='DustAway-Weixin-021-checks'
if(Get-ScheduledTask -TaskName $task -ErrorAction SilentlyContinue){throw 'Test task already exists'}
$source=@'
$ErrorActionPreference='Stop'
$root='C:\Users\MSN\AppData\Local\DustAwayAcceptance\weixin-021'
if(([Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Limited token required'}
$env:DUSTAWAY_WEIXIN_RUN='__RUN_ID__'
Remove-Item Env:DUSTAWAY_WEIXIN_EXECUTE -ErrorAction SilentlyContinue
foreach($rule in @('weixin.code','weixin.gpu')){
  $env:DUSTAWAY_WEIXIN_RULE=$rule
  $output=Join-Path $root ($env:DUSTAWAY_WEIXIN_RUN+'-'+$rule)
  if(Test-Path -LiteralPath ($output+'.log')){throw 'Run already exists'}
  $p=Start-Process -FilePath (Join-Path $root 'weixin-tests.exe') -ArgumentList @('--exact','weixin_cache::acceptance::production_weixin_engine_acceptance','--ignored','--nocapture') -WorkingDirectory $root -WindowStyle Hidden -PassThru -Wait -RedirectStandardOutput ($output+'.log') -RedirectStandardError ($output+'.err')
  [IO.File]::WriteAllText(($output+'.exit'),[string]$p.ExitCode)
  if($p.ExitCode -ne 0){throw 'Production boundary test failed'}
}
'@
$source=$source.Replace('__RUN_ID__',$RunId)
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($source))
$action=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand '+$encoded) -WorkingDirectory $root
$principal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $task -Action $action -Principal $principal -Settings (New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::FromMinutes(5))) | Out-Null
Start-ScheduledTask -TaskName $task
