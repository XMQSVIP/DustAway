$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
try{
 $taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
 if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
 if(Get-Process -Name brave,setup,brave-rules-driver,brave-rules-v2-driver,brave-inner-installer-154.1.96.60 -ErrorAction SilentlyContinue){throw 'Prior process remains'}
 if(Get-NetTCPConnection -State Listen -LocalPort 19839 -ErrorAction SilentlyContinue){throw 'Test server remains'}
 $taskPrepare=Get-Content -LiteralPath (Join-Path $taskRoot 'brave-reprepare-v2-result.json') -Raw|ConvertFrom-Json
 $taskReady=Get-Content -LiteralPath (Join-Path $taskRoot 'brave-real-ready-v4.json') -Raw|ConvertFrom-Json
 if($taskPrepare.ExitCode -ne 0 -or $taskPrepare.NativeExitCode -ne 0 -or $taskReady.Preparation -ne 'brave-reprepare-v2'){throw 'New preparation incomplete'}
 $taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'brave-seed-v4-manifest.json') -Raw|ConvertFrom-Json
 foreach($taskFile in $taskManifest.Files.PSObject.Properties){if($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name)).Hash -ne $taskFile.Value){throw 'Integrity mismatch'}}
 if(Test-Path -LiteralPath (Join-Path $taskRoot 'brave-evidence-v4-seed.started')){throw 'Already started; no replay'}
 $taskRun=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('brave-seed-v4.mjs','brave-evidence-v4') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'brave-seed-v4-run.log') -RedirectStandardError (Join-Path $taskRoot 'brave-seed-v4-error.log')
 if($null -eq $taskRun.ExitCode){throw 'Missing seed exit code'}
 [pscustomobject]@{ExitCode=$taskRun.ExitCode;Finished=(Get-Date).ToString('o');OrdinaryUser=$true;DeletionAttempted=$false}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $taskRoot 'brave-seed-v4-result.json') -Encoding UTF8
 exit $taskRun.ExitCode
}catch{$_|Out-String|Add-Content -LiteralPath (Join-Path $taskRoot 'brave-seed-v4-error.log') -Encoding UTF8;exit 99}
