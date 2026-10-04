$ErrorActionPreference='Stop'
$root=$PSScriptRoot
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $root -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup')){throw 'Wrong environment'}
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
$principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
$exe='C:\Program Files (x86)\DingDing\main\current\DingTalk.exe'
if((Get-FileHash -LiteralPath $exe).Hash -ne '5705DE940C8BF048792AF1E99B55C19F914FC1CA89B6DDB7AB3C4AC0595F69F4'){throw 'DingTalk identity changed'}
if(@(Get-CimInstance Win32_Process | Where-Object {$_.Name -match 'DingTalk|DingDing' -or $_.ExecutablePath -like '*\DingDing\*'}).Count){throw 'DingTalk is running'}
Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('dingtalk-remove.mjs','--continue-path-fix') -WorkingDirectory $root -WindowStyle Hidden -Wait -RedirectStandardOutput (Join-Path $root 'dingtalk-path-fix-run.log') -RedirectStandardError (Join-Path $root 'dingtalk-path-fix-error.log')
