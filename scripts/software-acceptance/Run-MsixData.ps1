# Ordinary interactive user only. Does not install/trust/uninstall a package or delete data.
param([Parameter(Mandatory=$true)][ValidateSet('Preflight','Seed','Verify','ObserveSeed','ObserveVerify')][string]$Stage)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$taskRoot = [IO.Path]::GetFullPath($PSScriptRoot)
if ($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or
    $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or
    [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8') {
    throw 'Wrong MSIX data acceptance environment'
}
$taskPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if ($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Ordinary user token required' }
if ([Diagnostics.Process]::GetCurrentProcess().SessionId -eq 0) { throw 'Interactive user session required' }
$taskDriver = Join-Path $taskRoot 'store_data_control.exe'
if ((Get-FileHash -LiteralPath $taskDriver -Algorithm SHA256).Hash -cne '5598F519976F571591350CA4F08AF7DE0F90748A903F6084281E1C6C0E9D99DD') { throw 'Unexpected controller build' }
$taskArguments = switch ($Stage) {
    'Preflight' { @('preflight') }
    'Seed' { @('seed') }
    'Verify' { @('verify') }
    'ObserveSeed' { @('observe', 'seed') }
    'ObserveVerify' { @('observe', 'verify') }
}
# Unique runner logs preserve every observation. Only the native controller may create
# the durable one-shot activation receipt; Observe never reactivates the fixture.
$taskPrefix = 'msix-data-run-' + $Stage.ToLowerInvariant() + '-' + [Guid]::NewGuid().ToString('N')
[pscustomobject]@{Time=(Get-Date).ToString('o');Stage=$Stage;Logs=$taskPrefix;OrdinaryUser=$true} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot ($taskPrefix + '.started.json')) -Encoding UTF8
# Windows PowerShell 5.1 may lose ExitCode on an asynchronous Start-Process object.
# -Wait retains the observed native exit code; null must never become PowerShell exit 0.
$taskProcess = Start-Process -FilePath $taskDriver -ArgumentList $taskArguments -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot ($taskPrefix + '.stdout.log')) -RedirectStandardError (Join-Path $taskRoot ($taskPrefix + '.stderr.log'))
if ($null -eq $taskProcess.ExitCode) { throw 'Controller exit code unavailable; do not report success or restart' }
[pscustomobject]@{Time=(Get-Date).ToString('o');Stage=$Stage;ControllerPid=$taskProcess.Id;ExitCode=$taskProcess.ExitCode;Logs=$taskPrefix;OrdinaryUser=$true;PackageInstalledByRunner=$false;UninstallExecuted=$false;DeletionExecuted=$false;SecuritySettingsChanged=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot ($taskPrefix + '.result.json')) -Encoding UTF8
exit $taskProcess.ExitCode
