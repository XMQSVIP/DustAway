$ErrorActionPreference='Stop'
if($env:COMPUTERNAME-ne'DESKTOP-24NJTT6'-or$env:USERNAME-ne'MSN'){throw 'Wrong host/user'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20261002-progress-visible-v2'
if(Test-Path -LiteralPath $taskRoot){throw 'Preserve existing evidence'}
Expand-Archive -LiteralPath (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20260930-software/progress-visible-v2-bundle.zip') -DestinationPath $taskRoot
if([IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-progress-visible'))-ne'DustAway.ProgressVisible.20261002.V2'){throw 'Missing marker'}
New-Item -ItemType Directory -Path (Join-Path $taskRoot 'artifacts/webdriver') | Out-Null
Copy-Item -LiteralPath (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20261001-cleanup-completion/artifacts/webdriver/msedgedriver.exe') -Destination (Join-Path $taskRoot 'artifacts/webdriver')
$taskName='DustAway-ProgressVisible-20261002-v2'
if(Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue){throw 'Already dispatched'}
$taskLaunch='$ErrorActionPreference="Stop"; $r="'+$taskRoot+'"; & ([scriptblock]::Create([IO.File]::ReadAllText((Join-Path $r "scripts/progress-visible/Run-Remote.ps1")))) -TestRoot $r'
$taskEncoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($taskLaunch))
$taskAction=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand '+$taskEncoded) -WorkingDirectory $taskRoot
$taskPrincipal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $taskName -Action $taskAction -Principal $taskPrincipal | Out-Null
Start-ScheduledTask -TaskName $taskName
@{task=$taskName;root=$taskRoot;sha256=(Get-FileHash -LiteralPath (Join-Path $taskRoot 'dustaway.exe')).Hash} | ConvertTo-Json
