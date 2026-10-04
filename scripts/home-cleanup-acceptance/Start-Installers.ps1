$ErrorActionPreference='Stop'
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or [IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong environment'}
foreach($product in @('DingTalk','Weixin')){
 $taskName='DustAway-HomeCleanup-'+$product+'-20261001'
 if((Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue) -or (Test-Path -LiteralPath (Join-Path $root ($product+'.install.started')))){throw 'Already launched; do not replay'}
 $action=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+(Join-Path $root 'Run-Installer.ps1')+'" -Product '+$product) -WorkingDirectory $root
 $principal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
 $settings=New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
 Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal -Settings $settings | Out-Null
 Start-ScheduledTask -TaskName $taskName
 [ordered]@{task=$taskName;time=(Get-Date -Format o);ordinary_user=$true;stage='interactive_installer_requested'} | ConvertTo-Json
}
