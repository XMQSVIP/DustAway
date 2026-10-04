$ErrorActionPreference='Stop'
if($env:COMPUTERNAME-ne'DESKTOP-24NJTT6'-or$env:USERNAME-ne'MSN'){throw 'Wrong host/user'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20261002-readonly-unlock-v1'
if(Test-Path -LiteralPath $taskRoot){throw 'Preserve existing evidence'}
Expand-Archive -LiteralPath (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20260930-software/readonly-unlock-v1-bundle.zip') -DestinationPath $taskRoot
if([IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-readonly-unlock'))-ne'DustAway.ReadonlyUnlock.20261002.V1'){throw 'Missing marker'}
New-Item -ItemType Directory -Path (Join-Path $taskRoot 'artifacts/webdriver') | Out-Null
Copy-Item -LiteralPath (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20261001-cleanup-completion/artifacts/webdriver/msedgedriver.exe') -Destination (Join-Path $taskRoot 'artifacts/webdriver')
@{root=$taskRoot;sha256=(Get-FileHash -LiteralPath (Join-Path $taskRoot 'dustaway.exe')).Hash;processes=@(Get-Process -Name dustaway,node,python,tauri-driver,msedgedriver,YunDetectService -ErrorAction SilentlyContinue | Select-Object ProcessName,Id)} | ConvertTo-Json
