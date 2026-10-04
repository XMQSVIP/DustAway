# Run by an ordinary-user interactive scheduled task on the dedicated test computer.
param([ValidateSet('msi','msix')][string[]]$Kinds=@('msi','msix'))
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
[Console]::OutputEncoding=[Text.Encoding]::UTF8
$taskRoot=$PSScriptRoot
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong fixture environment'}
Set-Location -LiteralPath $taskRoot
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Package acceptance must run with the ordinary user token'}
$taskResults=[Collections.Generic.List[object]]::new()
foreach($taskKind in $Kinds){
 try {
  if($taskKind -eq 'msi'){
   if(Test-Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{7C7E25E8-73B7-4D77-B8E7-7B4BB5650930}'){throw 'MSI fixture already installed; no replay'}
   $taskMsi=Start-Process -FilePath "$env:SystemRoot\System32\msiexec.exe" -ArgumentList @('/i',('"'+(Join-Path $taskRoot 'DustAwayAcceptance.msi')+'"'),'/qn','/norestart','/L*v',('"'+(Join-Path $taskRoot 'msi-install.log')+'"')) -WindowStyle Hidden -PassThru -Wait
   if($taskMsi.ExitCode -ne 0){throw ('MSI installation failed: '+$taskMsi.ExitCode)}
   if((Get-ItemProperty 'HKCU:\Software\DustAwayAcceptance\7c7e25e8').MSIInstalled -ne '1'){throw 'MSI component not installed'}
  } else {
   if(Get-AppxPackage -Name 'DustAway.Acceptance.7c7e25e8'){throw 'MSIX fixture already installed; no replay'}
   Add-AppxPackage -Path (Join-Path $taskRoot 'DustAwayAcceptance.msix') -ErrorAction Stop
   $taskPkg=Get-AppxPackage -Name 'DustAway.Acceptance.7c7e25e8'
   if(-not $taskPkg -or $taskPkg.Publisher -ne 'CN=DustAway Acceptance 7c7e25e8'){throw 'Unexpected package identity'}
   $taskData=Join-Path $env:LOCALAPPDATA ('Packages\'+$taskPkg.PackageFamilyName+'\LocalState')
   New-Item -ItemType Directory -Path $taskData -Force | Out-Null
   [IO.File]::WriteAllText((Join-Path $taskData 'acceptance-only-data.txt'),'This dedicated package owns this test data.')
  }
  $taskLog=Join-Path $taskRoot ($taskKind+'-run.log')
  # Process redirection avoids PowerShell 5.1 turning native stderr into an early terminating error.
  $taskRun=Start-Process -FilePath (Join-Path $taskRoot 'software_package_acceptance.exe') -ArgumentList $taskKind -WorkingDirectory $taskRoot -WindowStyle Hidden -PassThru -Wait -RedirectStandardOutput $taskLog -RedirectStandardError (Join-Path $taskRoot ($taskKind+'-error.log'))
  if($taskRun.ExitCode -ne 0){throw ('Native '+$taskKind+' acceptance failed: '+$taskRun.ExitCode)}
  if($taskKind -eq 'msi'){
   if((Get-ItemProperty 'HKCU:\Software\DustAwayAcceptance\7c7e25e8' -ErrorAction SilentlyContinue).MSIInstalled){throw 'MSI registry payload remains'}
  } else {
   if(Get-AppxPackage -Name 'DustAway.Acceptance.7c7e25e8'){throw 'MSIX remains registered'}
   if(Test-Path -LiteralPath (Join-Path $taskData 'acceptance-only-data.txt')){throw 'MSIX standard removal did not remove the dedicated app data'}
  }
  $taskResults.Add([pscustomobject]@{Kind=$taskKind;Result='passed';OrdinaryUser=$true})
 } catch {
  $taskResults.Add([pscustomobject]@{Kind=$taskKind;Result='failed';Error=$_.Exception.Message})
 }
 $taskSummaryName=if($Kinds.Count -eq 1){$Kinds[0]+'-run-summary.json'}else{'package-acceptance-summary.json'}
 $taskResults | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $taskRoot $taskSummaryName) -Encoding UTF8
}
[IO.File]::WriteAllText((Join-Path $taskRoot 'package-run-complete.txt'),(Get-Date).ToString('o'))
