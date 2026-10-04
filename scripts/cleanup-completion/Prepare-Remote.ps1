$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $env:USERNAME -ne 'MSN'){throw 'Wrong host/user'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-cleanup-completion'
$taskZip=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup\cleanup-completion-bundle.zip'
if(Test-Path -LiteralPath $taskRoot){throw 'Remote bundle already exists'}
Expand-Archive -LiteralPath $taskZip -DestinationPath $taskRoot
if([IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-completion')) -ne 'DustAway.CleanupCompletion.20261001'){throw 'Missing marker'}
$taskDriver=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software\firefox-desktop-delete-20261001-v2\artifacts\webdriver\msedgedriver.exe'
if((Get-Item -LiteralPath $taskDriver).VersionInfo.FileVersion -ne '154.0.4258.37'){throw 'Unexpected driver version'}
Copy-Item -LiteralPath $taskDriver -Destination (Join-Path $taskRoot 'artifacts/webdriver/msedgedriver.exe')
Get-FileHash -LiteralPath $taskZip,(Join-Path $taskRoot 'dustaway.exe'),(Join-Path $taskRoot 'artifacts/webdriver/msedgedriver.exe') | Select-Object Path,Hash | ConvertTo-Json
