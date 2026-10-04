$Phase='uv-postcheck'
$ErrorActionPreference='Stop'
$remote=@'
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
if($env:COMPUTERNAME-ne'DESKTOP-24NJTT6'){throw 'Wrong host'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20261002-space-relief'
if([IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-space-relief'))-ne'DustAway.SpaceRelief.20261002'){throw 'Wrong marker'}
$taskName='DustAway-SpaceRelief-20261002-encoded-__PHASE__'
if(Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue){throw 'Already dispatched; never replay'}
if(Test-Path -LiteralPath (Join-Path $taskRoot 'evidence/__PHASE__')){throw 'Evidence already exists: inspect instead of replaying'}
$launch='$ErrorActionPreference="Stop"; $r="'+$taskRoot+'"; & ([scriptblock]::Create([IO.File]::ReadAllText((Join-Path $r "scripts/space-relief/uv-postcheck.ps1")))) -TestRoot $r'
$launchEncoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($launch))
$taskAction=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand '+$launchEncoded) -WorkingDirectory $taskRoot
$taskPrincipal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $taskName -Action $taskAction -Principal $taskPrincipal | Out-Null
Start-ScheduledTask -TaskName $taskName
@{task=$taskName;sha256=(Get-FileHash -LiteralPath (Join-Path $taskRoot 'dustaway.exe')).Hash}|ConvertTo-Json
'@
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($remote.Replace('__PHASE__',$Phase)))
& 'C:\Program Files\Git\usr\bin\ssh.exe' -F /dev/null -i C:/Users/Administrator/.ssh/dustaway-desktop-24njtt6-ed25519 -o IdentitiesOnly=yes -o BatchMode=yes -o ConnectTimeout=8 -o StrictHostKeyChecking=yes -o UserKnownHostsFile=C:/Users/Administrator/.ssh/dustaway-desktop-24njtt6-known-hosts -o GlobalKnownHostsFile=/dev/null -o HostKeyAlgorithms=ssh-ed25519 -l MSN 100.69.113.49 "powershell.exe -NoProfile -NonInteractive -OutputFormat Text -EncodedCommand $encoded"
exit $LASTEXITCODE
