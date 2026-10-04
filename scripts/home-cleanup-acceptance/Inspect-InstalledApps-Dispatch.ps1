$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
& (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup\Inspect-InstalledApps.ps1')
