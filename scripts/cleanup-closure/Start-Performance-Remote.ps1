$ErrorActionPreference='Stop'
if($env:COMPUTERNAME-ne'DESKTOP-24NJTT6'-or$env:USERNAME-ne'MSN'){throw 'Wrong host/user'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20261002-cleanup-closure-performance'
if(Test-Path -LiteralPath $taskRoot){throw 'Preserve existing performance evidence'}
Expand-Archive -LiteralPath (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20261001-home-cleanup/cleanup-performance-bundle.zip') -DestinationPath $taskRoot
if((Get-FileHash -LiteralPath (Join-Path $taskRoot 'performance-current.exe')).Hash-ne'E32749BE04C1E50C1F9E46EB40D05841C1A89CA866C292AD2698C846C7A3B588'){throw 'Current hash mismatch'}
if((Get-FileHash -LiteralPath (Join-Path $taskRoot 'performance-baseline.exe')).Hash-ne'35DF718D704A6470E225FDFBF285D07DA6F8823278A8F832A16A24B7BE624CFB'){throw 'Baseline hash mismatch'}
$taskName='DustAway-CleanupClosure-20261002-perf-isolated'
if(Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue){throw 'Never replay task'}
$taskRun=@'
$ErrorActionPreference='Stop'
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20261002-cleanup-closure-performance'
$principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Limited required'}
if(Get-Process -Name dustaway,tauri-driver,msedgedriver,chrome -ErrorAction SilentlyContinue){throw 'Desktop busy'}
$env:DUSTAWAY_HOME_PERF_OUTPUT=Join-Path $taskRoot 'comparison'
$env:DUSTAWAY_HOME_PERF_BASELINE=Join-Path $taskRoot 'performance-baseline.exe'
$env:DUSTAWAY_HOME_PERF_CURRENT=Join-Path $taskRoot 'performance-current.exe'
$p=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList 'Compare-ScanPerformance.mjs' -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'stdout.log') -RedirectStandardError (Join-Path $taskRoot 'stderr.log')
@{exit_code=$p.ExitCode;ordinary_user=$true;finished=(Get-Date -Format o)}|ConvertTo-Json|Set-Content (Join-Path $taskRoot 'run.json') -Encoding UTF8
exit $p.ExitCode
'@
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($taskRun))
$action=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand '+$encoded) -WorkingDirectory $taskRoot
$principal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal | Out-Null
Start-ScheduledTask -TaskName $taskName
@{task=$taskName;root=$taskRoot}|ConvertTo-Json
