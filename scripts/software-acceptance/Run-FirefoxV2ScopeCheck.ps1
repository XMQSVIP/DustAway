param([ValidateSet('initial','version-corrected')][string]$Attempt = 'initial')
$ErrorActionPreference = 'Stop'
$taskPrefix = if ($Attempt -eq 'initial') { 'firefox-v2-scope' } else { 'firefox-v2-scope-version-corrected' }
$taskManifestName = if ($Attempt -eq 'initial') { 'firefox-v2-continuation-manifest.json' } else { 'firefox-v2-version-corrected-manifest.json' }
$taskRoot = [IO.Path]::GetFullPath($PSScriptRoot)
if ($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or
    [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8') { throw 'Wrong dedicated environment' }
$taskPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if ($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Expected ordinary token' }
if (Get-Process -Name firefox,Un_A,FirefoxSetup,software_firefox_v2_test,dustaway -ErrorAction SilentlyContinue) { throw 'A prior process remains active' }
$taskInstall = Join-Path $taskRoot 'firefox-real/application'
if (-not (Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe'))) { throw 'Existing test installation required; this script never installs' }
$taskReady = Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-real-ready.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if ([string]$taskReady.ProfileDirectory -notmatch '^[a-zA-Z0-9_-]+\.DustAwayAcceptance20260930$') { throw 'Invalid profile leaf' }
$taskProfile = Join-Path $env:APPDATA ('Mozilla/Firefox/Profiles/' + $taskReady.ProfileDirectory)
if ([IO.File]::ReadAllText((Join-Path $taskProfile '.dustaway-test-only')) -ne 'DustAwayFirefox20260930') { throw 'Profile marker missing' }
$taskSignature = Get-AuthenticodeSignature -LiteralPath (Join-Path $taskInstall 'firefox.exe')
if ($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'O=Mozilla Corporation') { throw 'Unexpected Firefox signature' }
$taskManifest = Get-Content -LiteralPath (Join-Path $taskRoot $taskManifestName) -Raw -Encoding UTF8 | ConvertFrom-Json
foreach ($taskFile in $taskManifest.Files.PSObject.Properties) {
    if ($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name) -Algorithm SHA256).Hash -ne $taskFile.Value) { throw 'Driver integrity check failed' }
}
$taskOnce = [IO.File]::Open((Join-Path $taskRoot ($taskPrefix+'-check.started')),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
[pscustomobject]@{Stage='scope_check_existing_installation';Attempt=$Attempt;Elevated=$false;Session=[Diagnostics.Process]::GetCurrentProcess().SessionId;Started=(Get-Date).ToString('o');InstallerReplayed=$false;UninstallExecuted=$false;ResidualDeletionExecuted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot ($taskPrefix+'-context.json')) -Encoding UTF8
$taskExit = 99
$taskBrowserExit = $null
$taskCheckExit = $null
try {
    # -Wait preserves the exit code in Windows PowerShell even for redirected children.
    $taskBrowser = Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('firefox-profile-check.mjs','retained',$taskPrefix) -WorkingDirectory $taskRoot -WindowStyle Hidden -PassThru -Wait -RedirectStandardOutput (Join-Path $taskRoot ($taskPrefix+'-browser-run.log')) -RedirectStandardError (Join-Path $taskRoot ($taskPrefix+'-browser-error.log'))
    $taskBrowserExit = $taskBrowser.ExitCode
    if ($taskBrowserExit -ne 0) { throw 'Browser verification did not finish successfully' }
    $taskCheck = Start-Process -FilePath (Join-Path $taskRoot 'software_firefox_v2_test.exe') -ArgumentList @('--ignored','--exact','software::firefox_scope_acceptance::real_firefox_v2_scope_preflight','--nocapture') -WorkingDirectory $taskRoot -WindowStyle Hidden -PassThru -Wait -RedirectStandardOutput (Join-Path $taskRoot ($taskPrefix+'-run.log')) -RedirectStandardError (Join-Path $taskRoot ($taskPrefix+'-error.log'))
    $taskCheckExit = $taskCheck.ExitCode
    if ($taskCheckExit -ne 0) { throw 'Independent scope verification did not finish successfully' }
    $taskExit = 0
} catch {
    $_ | Out-String | Set-Content -LiteralPath (Join-Path $taskRoot ($taskPrefix+'-check-error.log')) -Encoding UTF8
}
[pscustomobject]@{ExitCode=$taskExit;BrowserExitCode=$taskBrowserExit;ScopeCheckExitCode=$taskCheckExit;Finished=(Get-Date).ToString('o');InstallerReplayed=$false;UninstallExecuted=$false;ResidualDeletionExecuted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot ($taskPrefix+'-check-result.json')) -Encoding UTF8
exit $taskExit
