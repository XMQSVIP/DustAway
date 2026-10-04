param([Parameter(Mandatory=$true)][ValidateSet('dingtalk.http','dingtalk.code','baidu_netdisk.http','baidu_netdisk.code','baidu_netdisk.gpu')][string]$Rule)
$ErrorActionPreference='Stop'
$remote=@'
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
if(@(Get-ScheduledTask | Where-Object {$_.TaskName -like 'DustAway-HomeCleanup-*' -and $_.State -eq 'Running'}).Count){throw 'Another test action is running'}
$rule='__RULE__'
if(Test-Path -LiteralPath (Join-Path $root ('real-clean-'+$rule+'.started'))){throw 'Do not replay cleanup'}
$name='DustAway-HomeCleanup-Clean-'+$rule+'-'+(Get-Date -Format 'HHmmss')
$action=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+(Join-Path $root 'Run-RealCleanup.ps1')+'" -Rule '+$rule) -WorkingDirectory $root
$principal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
$settings=New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
Register-ScheduledTask -TaskName $name -Action $action -Principal $principal -Settings $settings | Out-Null
Start-ScheduledTask -TaskName $name
@{time=(Get-Date -Format o);rule=$rule;task=$name;stage='requested'} | ConvertTo-Json
'@
$remote=$remote.Replace('__RULE__',$Rule)
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($remote))
$sshArgs=@('-F','/dev/null','-i',(Join-Path $env:USERPROFILE '.ssh/dustaway-desktop-24njtt6-ed25519').Replace('\','/'),'-o','IdentitiesOnly=yes','-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o',('UserKnownHostsFile='+(Join-Path $env:USERPROFILE '.ssh/dustaway-desktop-24njtt6-known-hosts').Replace('\','/')),'-o','GlobalKnownHostsFile=/dev/null','-o','HostKeyAlgorithms=ssh-ed25519','-o','ConnectTimeout=10')
& 'C:\Program Files\Git\usr\bin\ssh.exe' @sshArgs MSN@100.69.113.49 "powershell.exe -NoProfile -NonInteractive -OutputFormat Text -EncodedCommand $encoded"
if($LASTEXITCODE -ne 0){throw 'Cleanup dispatch refused'}
