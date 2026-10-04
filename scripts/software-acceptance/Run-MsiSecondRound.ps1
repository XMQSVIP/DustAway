# Two NEW fixture identities; never retries the earlier MSI or any recorded uninstall.
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
Set-Location -LiteralPath $taskRoot
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Expected ordinary token'}
$taskLock=[IO.File]::Open((Join-Path $taskRoot 'msi-second-round.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskLock.Dispose()
$taskBoot=(Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToString('o')
$taskStarted=Get-Date
$taskResults=[Collections.Generic.List[object]]::new()
[pscustomobject]@{Account=[Security.Principal.WindowsIdentity]::GetCurrent().Name;Elevated=$false;Session=[Diagnostics.Process]::GetCurrentProcess().SessionId;Started=$taskStarted.ToString('o');BootTime=$taskBoot} | ConvertTo-Json | Set-Content -LiteralPath 'msi-second-round-context.json' -Encoding UTF8
try {
 $taskInstaller=New-Object -ComObject WindowsInstaller.Installer
 foreach($taskCase in @(
  @{Kind='msi-completion';File='DustAwayAcceptanceCompletion.msi';Product='{7C7E25E8-73B7-4D77-B8E7-7B4BB5650931}';Value='MSICompletionInstalled';Hash='A18B992760D20357E4D881926EC16528BD4168EC6B250074417045630471BE97'},
  @{Kind='msi-restart';File='DustAwayAcceptanceRestart.msi';Product='{7C7E25E8-73B7-4D77-B8E7-7B4BB5650932}';Value='MSIRestartInstalled';Hash='6E9B17C4FD2A9C08945AF44986596993520F18F07AD161A23745363633DDF60D'}
 )){
  if(Test-Path -LiteralPath ($taskCase.Kind+'-acceptance.sqlite')){throw 'Uninstall journal already exists; no replay'}
  if($taskInstaller.ProductState($taskCase.Product) -ne -1){throw 'Sample already registered; no reinstall'}
  if((Get-FileHash -LiteralPath $taskCase.File -Algorithm SHA256).Hash -ne $taskCase.Hash){throw 'Wrong MSI sample'}
  $taskInstall=Start-Process -FilePath "$env:SystemRoot\System32\msiexec.exe" -ArgumentList @('/i',('"'+(Join-Path $taskRoot $taskCase.File)+'"'),'/qn','/norestart','/L*v',('"'+(Join-Path $taskRoot ($taskCase.Kind+'-install.log'))+'"')) -WindowStyle Hidden -PassThru -Wait
  if($taskInstall.ExitCode -ne 0 -or $taskInstaller.ProductState($taskCase.Product) -ne 5){throw ('Fixture install failed: '+$taskInstall.ExitCode)}
  if((Get-ItemProperty 'HKCU:\Software\DustAwayAcceptance\7c7e25e8').($taskCase.Value) -ne '1'){throw 'Sample payload missing'}
  $taskRun=Start-Process -FilePath (Join-Path $taskRoot 'software_package_acceptance.exe') -ArgumentList $taskCase.Kind -WorkingDirectory $taskRoot -WindowStyle Hidden -PassThru -Wait -RedirectStandardOutput ($taskCase.Kind+'-run.log') -RedirectStandardError ($taskCase.Kind+'-error.log')
  if($taskRun.ExitCode -ne 0){throw ('Native '+$taskCase.Kind+' verification failed; no retry: '+$taskRun.ExitCode)}
  if($taskInstaller.ProductState($taskCase.Product) -ne -1){throw 'Product still registered'}
  if((Get-ItemProperty 'HKCU:\Software\DustAwayAcceptance\7c7e25e8' -ErrorAction SilentlyContinue).($taskCase.Value)){throw 'Sample payload remains'}
  $taskResults.Add([pscustomobject]@{Kind=$taskCase.Kind;Result='passed';ProductRemoved=$true;PayloadRemoved=$true})
 }
 $taskStatus='passed'
} catch {
 $_ | Out-String | Set-Content -LiteralPath 'msi-second-round-error.log' -Encoding UTF8
 $taskStatus='failed'
} finally {
 $taskBootAfter=(Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToString('o')
 [pscustomobject]@{Result=$taskStatus;Started=$taskStarted.ToString('o');Finished=(Get-Date).ToString('o');Cases=@($taskResults.ToArray());NoRestart=($taskBoot -eq $taskBootAfter);ProductionResidualDeletionEnabled=$false} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath 'msi-second-round-result.json' -Encoding UTF8
}
