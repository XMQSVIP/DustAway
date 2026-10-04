$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath($PSScriptRoot)
if ($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or
    [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8') { throw 'Wrong dedicated environment' }
$taskPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if ($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Ordinary user required' }
if (Get-Process -Name firefox,Un_A,FirefoxSetup,software_firefox_rules_test,dustaway -ErrorAction SilentlyContinue) { throw 'Prior process remains; do not replay' }
if (-not (Test-Path -LiteralPath (Join-Path $taskRoot 'firefox-real/application/firefox.exe'))) { throw 'Existing dedicated installation required' }
$taskManifest = Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-rules-manifest.json') -Raw -Encoding UTF8 | ConvertFrom-Json
foreach ($taskFile in $taskManifest.Files.PSObject.Properties) {
    if ($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name) -Algorithm SHA256).Hash -ne $taskFile.Value) { throw 'Driver integrity mismatch' }
}
$taskOnce = [IO.File]::Open((Join-Path $taskRoot 'firefox-rules-run.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
[pscustomobject]@{Elevated=$false;Session=[Diagnostics.Process]::GetCurrentProcess().SessionId;Started=(Get-Date).ToString('o');Purpose='Real independent residual-rule evidence';VendorSilentUninstallForDataScenario=$true;InteractiveLauncherTest=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-rules-context.json') -Encoding UTF8
$taskExit = 99
try {
    $taskRun = Start-Process -FilePath (Join-Path $taskRoot 'software_firefox_rules_test.exe') -ArgumentList @('--ignored','--exact','software::firefox_rules_acceptance::real_firefox_independent_rules','--nocapture') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'firefox-rules-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-rules-error.log')
    $taskExit = $taskRun.ExitCode
} catch { $_ | Out-String | Add-Content -LiteralPath (Join-Path $taskRoot 'firefox-rules-error.log') }
[pscustomobject]@{ExitCode=$taskExit;Finished=(Get-Date).ToString('o');InteractiveLauncherTest=$false;ProductionApprovalAdded=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-rules-result.json') -Encoding UTF8
exit $taskExit
