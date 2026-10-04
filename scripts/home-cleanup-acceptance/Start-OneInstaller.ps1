param([Parameter(Mandatory=$true)][ValidateSet('DingTalk','BaiduNetdisk','Weixin')][string]$Product,[ValidateRange(1,2)][int]$Attempt=1)
$ErrorActionPreference='Stop'
$remote=@'
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong environment'}
$product='__PRODUCT__';$attempt=__ATTEMPT__
$taskName='DustAway-HomeCleanup-'+$product+'-20261001'+$(if($attempt -gt 1){'-V'+$attempt}else{''})
if((Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue) -or (Test-Path -LiteralPath (Join-Path $root ($product+'.install.started')))){throw 'Already started; do not replay'}
if($attempt -gt 1){
 $prior=Get-ScheduledTask -TaskName ('DustAway-HomeCleanup-'+$product+'-20261001')
 if($prior.State -ne 'Ready' -or ($prior | Get-ScheduledTaskInfo).LastTaskResult -ne 1){throw 'Expected prelaunch failure missing'}
 if(Test-Path -LiteralPath (Join-Path $root ($product+'.install-launch.json'))){throw 'Prior installer launched; do not repeat'}
}
$manifest=Get-Content -LiteralPath (Join-Path $root ($product+'.manifest.json')) -Raw | ConvertFrom-Json
if($manifest.file -ne ($product+'.exe') -or (Get-FileHash -LiteralPath (Join-Path $root $manifest.file)).Hash -ne $manifest.sha256){throw 'Transfer incomplete or changed'}
$action=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+(Join-Path $root 'Run-Installer.ps1')+'" -Product '+$product) -WorkingDirectory $root
$principal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
$settings=New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal -Settings $settings | Out-Null
Start-ScheduledTask -TaskName $taskName
[ordered]@{time=(Get-Date -Format o);task=$taskName;ordinary_user=$true;stage='interactive_installer_requested'} | ConvertTo-Json
'@
$remote=$remote.Replace('__PRODUCT__',$Product).Replace('__ATTEMPT__',[string]$Attempt)
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($remote))
$sshArgs=@('-F','/dev/null','-i',(Join-Path $env:USERPROFILE '.ssh/dustaway-desktop-24njtt6-ed25519').Replace('\','/'),'-o','IdentitiesOnly=yes','-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o',('UserKnownHostsFile='+(Join-Path $env:USERPROFILE '.ssh/dustaway-desktop-24njtt6-known-hosts').Replace('\','/')),'-o','GlobalKnownHostsFile=/dev/null','-o','HostKeyAlgorithms=ssh-ed25519','-o','ConnectTimeout=10')
& 'C:\Program Files\Git\usr\bin\ssh.exe' @sshArgs MSN@100.69.113.49 "powershell.exe -NoProfile -NonInteractive -OutputFormat Text -EncodedCommand $encoded"
if($LASTEXITCODE -ne 0){throw 'Installer dispatch refused'}
