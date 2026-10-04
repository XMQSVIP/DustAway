$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
try{
 $taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
 if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
 if(Get-Process -Name brave,BraveBrowserStandaloneSilentSetup -ErrorAction SilentlyContinue){throw 'Prior vendor process remains'}
 if(Get-NetTCPConnection -State Listen -LocalPort 19839 -ErrorAction SilentlyContinue){throw 'Test server remains'}
 $taskPrepare=Get-Content -LiteralPath (Join-Path $taskRoot 'brave-inner-prepare-result.json') -Raw | ConvertFrom-Json
 $taskReady=Get-Content -LiteralPath (Join-Path $taskRoot 'brave-real-ready.json') -Raw | ConvertFrom-Json
 if($taskPrepare.ExitCode -ne 0 -or $taskPrepare.NativeExitCode -ne 0 -or $taskReady.Preparation -ne 'brave-inner-prepare'){throw 'Preparation incomplete'}
 $taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'brave-seed-manifest.json') -Raw | ConvertFrom-Json
 foreach($taskFile in $taskManifest.Files.PSObject.Properties){if($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name)).Hash -ne $taskFile.Value){throw 'Driver integrity mismatch'}}
 if(Test-Path -LiteralPath (Join-Path $taskRoot 'brave-evidence-seed.started')){throw 'Seed already started; do not replay'}
 $taskBase=Join-Path $env:LOCALAPPDATA 'BraveSoftware'
 if([IO.File]::ReadAllText((Join-Path $taskBase '.dustaway-test-only')) -ne 'DustAwayBrave20261001'){throw 'Wrong product marker'}
 $taskData=Join-Path $taskBase 'Brave-Browser\User Data'
 $taskMarker=Join-Path $taskData '.dustaway-test-only'
 if((Test-Path -LiteralPath $taskData) -and -not (Test-Path -LiteralPath $taskMarker)){
  for($taskParent=Get-Item -LiteralPath $taskData; $null -ne $taskParent; $taskParent=$taskParent.Parent){if($taskParent.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Linked data parent refused'}}
  $taskInitial=@(Get-ChildItem -LiteralPath $taskData -Force)
  $taskStarted=[DateTimeOffset](Get-Content -LiteralPath (Join-Path $taskRoot 'brave-inner-prepare-status.json') -Raw | ConvertFrom-Json).Started
  $taskFinished=[DateTimeOffset]$taskPrepare.Finished
  if($taskInitial.Count -ne 1 -or $taskInitial[0].Name -ne 'Crashpad' -or -not $taskInitial[0].PSIsContainer -or ($taskInitial[0].Attributes -band [IO.FileAttributes]::ReparsePoint) -or $taskInitial[0].CreationTimeUtc -lt $taskStarted.UtcDateTime -or $taskInitial[0].CreationTimeUtc -gt $taskFinished.UtcDateTime){throw 'Unexpected preexisting profile content'}
  [pscustomobject]@{OnlyInstallerCrashpad=$true;CreatedDuringVerifiedInstall=$true;PersonalProfilePresent=$false;ContentsRead=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'brave-seed-initial-directory.json') -Encoding UTF8
  $taskMarkerFile=[IO.File]::Open($taskMarker,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
  try{$taskBytes=[Text.Encoding]::UTF8.GetBytes('DustAwayBrave20261001');$taskMarkerFile.Write($taskBytes,0,$taskBytes.Length)}finally{$taskMarkerFile.Dispose()}
 }
 $taskRun=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('brave-seed.mjs','brave-evidence') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'brave-seed-run.log') -RedirectStandardError (Join-Path $taskRoot 'brave-seed-error.log')
 if($null -eq $taskRun.ExitCode){throw 'Missing seed exit code'}
 [pscustomobject]@{ExitCode=$taskRun.ExitCode;Finished=(Get-Date).ToString('o');OrdinaryUser=$true;DeletionAttempted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'brave-seed-result.json') -Encoding UTF8
 exit $taskRun.ExitCode
}catch{$_ | Out-String | Add-Content -LiteralPath (Join-Path $taskRoot 'brave-seed-error.log') -Encoding UTF8;exit 99}
