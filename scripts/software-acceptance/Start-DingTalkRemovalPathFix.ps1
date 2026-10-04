$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
$prior=[IO.File]::ReadAllText((Join-Path $root 'dingtalk-removal-20261001\status.json')) | ConvertFrom-Json
if($prior.phase -ne 'needs_review' -or $prior.submitted -ne $false -or (Test-Path -LiteralPath (Join-Path $root 'dingtalk-removal-20261001\uninstall-submit.started'))){throw 'Prior operation may have submitted: do not repeat'}
$name='DustAway-DingTalk-Removal-PathFix-20261001'
if((Test-Path -LiteralPath (Join-Path $root 'dingtalk-removal-path-fix')) -or (Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue)){throw 'Already requested'}
$manifest=[IO.File]::ReadAllText((Join-Path $root 'dingtalk-path-build.json')) | ConvertFrom-Json
if($manifest.fixture -ne $false -or (Get-FileHash -LiteralPath (Join-Path $root 'dustaway-uninstall-compatible.exe')).Hash -ne $manifest.sha256){throw 'Build mismatch'}
$action=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+(Join-Path $root 'Run-DingTalkRemovalPathFix.ps1')+'"') -WorkingDirectory $root
$principal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $name -Action $action -Principal $principal | Out-Null
Start-ScheduledTask -TaskName $name
@{time=(Get-Date -Format o);task=$name;stage='requested';previous_uninstall_submitted=$false} | ConvertTo-Json
