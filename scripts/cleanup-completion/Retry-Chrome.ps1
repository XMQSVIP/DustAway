$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-cleanup-completion'
if(Get-ChildItem -LiteralPath $taskRoot -Filter '*.cleanup-started'){throw 'A deletion was started; inspect it instead of replaying'}
$taskOld=Join-Path $taskRoot 'scripts/cleanup-completion/chrome-home.mjs'
Copy-Item -LiteralPath $taskOld -Destination (Join-Path $taskRoot 'evidence/chrome-first/original-chrome-home.mjs')
Copy-Item -LiteralPath (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup\chrome-home.mjs') -Destination $taskOld
& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $taskRoot 'scripts/cleanup-completion/Start-Remote.ps1') -Phase chrome -Run native-receipt
