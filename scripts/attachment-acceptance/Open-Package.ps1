$ErrorActionPreference = 'Stop'
$taskRoot = Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261002-attachments'
if ($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $PSScriptRoot -ne $taskRoot) { throw 'Wrong environment' }
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Ordinary user required' }
$exe = Join-Path $taskRoot 'portable\DustAway.exe'
$manifest = Get-Content -LiteralPath (Join-Path $taskRoot 'portable\package.json') -Raw | ConvertFrom-Json
if ((Get-FileHash -LiteralPath $exe).Hash -ne $manifest.sha256) { throw 'Package hash differs' }
Start-Process -FilePath $exe -WorkingDirectory (Split-Path -LiteralPath $exe) -WindowStyle Hidden
