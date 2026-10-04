$ErrorActionPreference='Stop'
$root=$PSScriptRoot
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $root -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup')){throw 'Wrong environment'}
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
$principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList 'dingtalk-remove.mjs' -WorkingDirectory $root -WindowStyle Hidden -Wait -RedirectStandardOutput (Join-Path $root 'dingtalk-removal-run.log') -RedirectStandardError (Join-Path $root 'dingtalk-removal-error.log')
