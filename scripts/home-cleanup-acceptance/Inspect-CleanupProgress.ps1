$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
@{time=(Get-Date -Format o);started=@(Get-ChildItem -LiteralPath $root -Filter 'real-clean-*.started' | Select-Object Name,Length,LastWriteTime);processes=@(Get-CimInstance Win32_Process -Filter "Name='dustaway-real-app-tests.exe'" | Select-Object ProcessId,KernelModeTime,UserModeTime,ReadTransferCount,WriteTransferCount,WorkingSetSize)} | ConvertTo-Json -Depth 4
