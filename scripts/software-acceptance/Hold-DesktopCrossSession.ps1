# Run once through the authorized SSH session. Only creates/releases its own pipe-waiting child.
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong fixture environment'}
if([Diagnostics.Process]::GetCurrentProcess().SessionId -ne 0){throw 'Expected the dedicated SSH session, not the desktop session'}
$taskDriver=Join-Path $taskRoot 'desktop-process-driver.exe'
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'desktop-process-driver-manifest.json') -Raw | ConvertFrom-Json
if((Get-FileHash -LiteralPath $taskDriver).Hash -cne $taskManifest.DriverSha256){throw 'Driver digest mismatch'}
$taskFixture=Join-Path $taskRoot 'desktop-process-cross-session-20261001'
if(Test-Path -LiteralPath $taskFixture){throw 'Fixture exists; never replay this parent'}
if(Test-Path -LiteralPath (Join-Path $taskRoot 'desktop-process-cross-session-context.json')){throw 'Context already exists'}
New-Item -ItemType Directory -Path $taskFixture | Out-Null
$taskToken=[Guid]::NewGuid().ToString()
[IO.File]::WriteAllText((Join-Path $taskFixture '.dustaway-process-fixture'),$taskToken)
$taskExe=Join-Path $taskFixture 'desktop-cross-session-child.exe'
Copy-Item -LiteralPath $taskDriver -Destination $taskExe
$taskInfo=New-Object Diagnostics.ProcessStartInfo
$taskInfo.FileName=$taskExe
$taskInfo.Arguments='--ignored --exact software::store_processes::acceptance::process_lifetime_child --nocapture'
$taskInfo.WorkingDirectory=$taskFixture
$taskInfo.UseShellExecute=$false
$taskInfo.CreateNoWindow=$true
$taskInfo.RedirectStandardInput=$true
$taskInfo.RedirectStandardOutput=$true
$taskInfo.RedirectStandardError=$true
$taskInfo.EnvironmentVariables['DUSTAWAY_STORE_PROCESS_CHILD']=$taskToken
$taskProcess=New-Object Diagnostics.Process
$taskProcess.StartInfo=$taskInfo
if(-not $taskProcess.Start()){throw 'Child did not start'}
$null=$taskProcess.Handle
$taskTrigger='script_error'
try {
 $taskReadyDeadline=(Get-Date).AddSeconds(15)
 while(-not(Test-Path -LiteralPath (Join-Path $taskFixture 'ready'))){
  if($taskProcess.HasExited){throw 'Child exited before readiness'}
  if((Get-Date) -gt $taskReadyDeadline){throw 'Child readiness timeout; no restart'}
  Start-Sleep -Milliseconds 100
 }
 if([IO.File]::ReadAllText((Join-Path $taskFixture 'ready')) -cne $taskToken){throw 'Child readiness token mismatch'}
 $taskContext=[ordered]@{pid=$taskProcess.Id;session=$taskProcess.SessionId;token=$taskToken;time=(Get-Date).ToString('o')}
 [IO.File]::WriteAllText((Join-Path $taskRoot 'desktop-process-cross-session-context.json'),($taskContext | ConvertTo-Json),(New-Object Text.UTF8Encoding($false)))
 'Dedicated cross-session child is live; waiting for its ordinary-user observer.'
 $taskDeadline=(Get-Date).AddSeconds(180)
 $taskRelease=Join-Path $taskFixture 'release'
 while(-not(Test-Path -LiteralPath $taskRelease)){
  if($taskProcess.HasExited){throw 'Child exited before observation'}
  if((Get-Date) -gt $taskDeadline){$taskTrigger='timeout';break}
  Start-Sleep -Milliseconds 100
 }
 if(Test-Path -LiteralPath $taskRelease){
  if([IO.File]::ReadAllText($taskRelease) -cne $taskToken){throw 'Observer release token differs'}
  $taskTrigger='controller_release'
 }
} finally {
 if(-not $taskProcess.HasExited){$taskProcess.StandardInput.WriteLine($taskToken);$taskProcess.StandardInput.Flush();$taskProcess.StandardInput.Close()}
 if(-not $taskProcess.WaitForExit(10000)){throw 'Child still live after release; do not terminate or restart'}
 $taskExit=$taskProcess.ExitCode
 if($null -eq $taskExit){throw 'Child exit status unavailable'}
 [IO.File]::WriteAllText((Join-Path $taskRoot 'desktop-process-cross-session-child.stdout.log'),$taskProcess.StandardOutput.ReadToEnd())
 [IO.File]::WriteAllText((Join-Path $taskRoot 'desktop-process-cross-session-child.stderr.log'),$taskProcess.StandardError.ReadToEnd())
 [pscustomobject]@{Time=(Get-Date).ToString('o');ChildPid=$taskProcess.Id;ChildExitCode=$taskExit;Trigger=$taskTrigger;ForcedTermination=$false;PackageOperations=$false;SecuritySettingsChanged=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'desktop-process-cross-session-parent-result.json') -Encoding UTF8
}
if($taskExit -ne 0 -or $taskTrigger -ne 'controller_release'){throw 'Cross-session observation did not complete normally'}
