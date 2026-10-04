$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
if((Test-Path -LiteralPath (Join-Path $root 'dingtalk-removal-20261001')) -or (Get-ScheduledTask -TaskName 'DustAway-DingTalk-Removal-20261001' -ErrorAction SilentlyContinue)){throw 'Already requested: inspect existing session'}
if(@(Get-CimInstance Win32_Process | Where-Object {$_.Name -match 'DingTalk|DingDing|dustaway|tauri-driver|msedgedriver'}).Count){throw 'Relevant process is running'}
if(Get-NetTCPConnection -State Listen -LocalPort 4444,4445,19837 -ErrorAction SilentlyContinue){throw 'Driver port in use'}
$exe='C:\Program Files (x86)\DingDing\main\current\DingTalk.exe'
if((Get-FileHash -LiteralPath $exe).Hash -ne '5705DE940C8BF048792AF1E99B55C19F914FC1CA89B6DDB7AB3C4AC0595F69F4'){throw 'DingTalk identity changed'}
$sig=Get-AuthenticodeSignature -LiteralPath $exe
if($sig.Status -ne 'Valid' -or $sig.SignerCertificate.Subject -notmatch 'DingTalk Technology Co.,Ltd.'){throw 'Unverified DingTalk signature'}
$action=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+(Join-Path $root 'Run-DingTalkRemoval.ps1')+'"') -WorkingDirectory $root
$principal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName 'DustAway-DingTalk-Removal-20261001' -Action $action -Principal $principal | Out-Null
Start-ScheduledTask -TaskName 'DustAway-DingTalk-Removal-20261001'
@{time=(Get-Date -Format o);task='DustAway-DingTalk-Removal-20261001';stage='requested'} | ConvertTo-Json
