param([Parameter(Mandatory=$true)][ValidateSet('Before','After')][string]$Phase)
$ErrorActionPreference='Stop'
$remote=@'
$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong test root'}
if(@(Get-ScheduledTask | Where-Object {$_.TaskName -like 'DustAway-HomeCleanup-*' -and $_.State -eq 'Running'}).Count){throw 'Another acceptance operation is active'}
$phase='__PHASE__'
if(Test-Path -LiteralPath (Join-Path $root ('ordinary-http-protection-'+$phase.ToLowerInvariant()+'.json'))){throw 'Checkpoint already recorded; inspect original receipt'}
$name='DustAway-HomeCleanup-OrdinaryProtection-'+$phase+'-'+(Get-Date -Format 'HHmmss')
$action=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+(Join-Path $root 'Run-OrdinaryProtection.ps1')+'" -Phase '+$phase) -WorkingDirectory $root
$principal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $name -Action $action -Principal $principal | Out-Null
Start-ScheduledTask -TaskName $name
@{time=(Get-Date -Format o);task=$name;phase=$phase;stage='requested';deletion_attempted=$false} | ConvertTo-Json
'@
$remote=$remote.Replace('__PHASE__',$Phase)
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($remote))
$sshArgs=@('-F','/dev/null','-i',(Join-Path $env:USERPROFILE '.ssh/dustaway-desktop-24njtt6-ed25519').Replace('\','/'),'-o','IdentitiesOnly=yes','-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o',('UserKnownHostsFile='+(Join-Path $env:USERPROFILE '.ssh/dustaway-desktop-24njtt6-known-hosts').Replace('\','/')),'-o','GlobalKnownHostsFile=/dev/null','-o','HostKeyAlgorithms=ssh-ed25519','-o','ConnectTimeout=10')
& 'C:\Program Files\Git\usr\bin\ssh.exe' @sshArgs MSN@100.69.113.49 "powershell.exe -NoProfile -NonInteractive -OutputFormat Text -EncodedCommand $encoded"
if($LASTEXITCODE -ne 0){throw 'Checkpoint dispatch failed'}
