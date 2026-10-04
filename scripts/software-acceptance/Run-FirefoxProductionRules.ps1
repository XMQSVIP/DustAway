$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath($PSScriptRoot)
if ($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or
    [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8') { throw 'Wrong dedicated environment' }
$taskPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if ($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Ordinary user required' }
if (Get-Process -Name firefox,Un_A,FirefoxSetup,software_firefox_rules_test,software_firefox_production_test,dustaway -ErrorAction SilentlyContinue) { throw 'Prior process remains; inspect before retry' }
$taskInstall = Join-Path $taskRoot 'firefox-real/application'
if (Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe')) { throw 'Prior test installation remains' }
$taskPrior = Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-rules-result.json') -Raw | ConvertFrom-Json
if ($taskPrior.ExitCode -ne 101 -or [IO.File]::ReadAllText((Join-Path $taskRoot 'firefox-rules-error.log')).Trim() -ne 'Error: no actual vendor file for rule') { throw 'Unexpected previous result' }
$taskRules = Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-rules-20261001-partial.json') -Raw | ConvertFrom-Json
if ($taskRules.Count -ne 6 -or @($taskRules | Where-Object { $_.deleted_files -lt 1 -or -not $_.unselected_hashes_unchanged }).Count -gt 0) { throw 'Incomplete per-rule evidence' }
$taskReady = Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-real-ready.json') -Raw | ConvertFrom-Json
if ([string]$taskReady.ProfileDirectory -notmatch '^[a-zA-Z0-9_-]+\.DustAwayAcceptance20260930$') { throw 'Wrong profile leaf' }
$taskProfile = [IO.Path]::GetFullPath((Join-Path $env:APPDATA ('Mozilla/Firefox/Profiles/' + $taskReady.ProfileDirectory)))
if ([IO.File]::ReadAllText((Join-Path $taskProfile '.dustaway-test-only')) -ne 'DustAwayFirefox20260930') { throw 'Profile marker missing' }
foreach ($taskLeaf in @('prefs.js','user.js','cookies.sqlite','places.sqlite')) {
    if (Test-Path -LiteralPath (Join-Path $taskProfile $taskLeaf)) { throw 'Earlier deletion phase or fixture state differs' }
}
$taskInstaller = Join-Path $taskRoot 'FirefoxSetup.exe'
if ((Get-FileHash -LiteralPath $taskInstaller -Algorithm SHA256).Hash -ne 'DF6F7046D53FF17403E49092C0FE830E24CABD8DACBFAD3225870955908FCCF9') { throw 'Installer hash mismatch' }
$taskSignature = Get-AuthenticodeSignature -LiteralPath $taskInstaller
if ($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'O=Mozilla Corporation') { throw 'Installer signature mismatch' }
$taskManifest = Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-production-manifest.json') -Raw | ConvertFrom-Json
foreach ($taskFile in $taskManifest.Files.PSObject.Properties) {
    if ($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name) -Algorithm SHA256).Hash -ne $taskFile.Value) { throw 'Driver integrity mismatch' }
}
$taskOnce = [IO.File]::Open((Join-Path $taskRoot 'firefox-production-run.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
[pscustomobject]@{Started=(Get-Date).ToString('o');Elevated=$false;Session=[Diagnostics.Process]::GetCurrentProcess().SessionId;OldEvidenceReplayed=$false;InteractiveLauncherTest=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-production-context.json') -Encoding UTF8
$taskExit = 99
try {
    # Remove only the exact one-file scope created by the earlier test; no recursive operation.
    $taskNewScope = [IO.Path]::GetFullPath((Join-Path $taskProfile 'sessionstore-backups'))
    if (-not $taskNewScope.StartsWith($taskProfile+'\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Scope escaped profile' }
    $taskChildren = @(Get-ChildItem -LiteralPath $taskNewScope -Force)
    if ($taskChildren.Count -ne 1 -or $taskChildren[0].Name -ne '.dustaway-new-scope' -or
        [IO.File]::ReadAllText($taskChildren[0].FullName) -ne 'test-only scope created after preparation') { throw 'New-scope witness changed' }
    Remove-Item -LiteralPath $taskChildren[0].FullName
    [IO.Directory]::Delete($taskNewScope,$false)
    $taskRun = Start-Process -FilePath $taskInstaller -ArgumentList @('/S',('/InstallDirectoryPath="'+$taskInstall+'"'),'/MaintenanceService=false','/RegisterDefaultAgent=false','/DesktopShortcut=false','/StartMenuShortcuts=false','/TaskbarShortcut=false','/PreventRebootRequired=true') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru
    if ($taskRun.ExitCode -ne 0 -or -not (Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe'))) { throw 'New fixture installation failed' }
    [IO.File]::WriteAllText((Join-Path $taskProfile 'user.js'),'user_pref("browser.shell.checkDefaultBrowser", false);'+[Environment]::NewLine+'user_pref("browser.startup.homepage_override.mstone", "ignore");')
    $taskSeed = Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('firefox-profile-check.mjs','seed','firefox-production') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'firefox-production-seed-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-production-seed-error.log')
    if ($taskSeed.ExitCode -ne 0) { throw 'New browser test data not verified' }
    $taskCheck = Start-Process -FilePath (Join-Path $taskRoot 'software_firefox_production_test.exe') -ArgumentList @('--ignored','--exact','software::firefox_rules_acceptance::real_firefox_production_rules','--nocapture') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'firefox-production-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-production-error.log')
    $taskExit = $taskCheck.ExitCode
} catch { $_ | Out-String | Add-Content -LiteralPath (Join-Path $taskRoot 'firefox-production-error.log') }
[pscustomobject]@{ExitCode=$taskExit;Finished=(Get-Date).ToString('o');InteractiveLauncherTest=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-production-result.json') -Encoding UTF8
exit $taskExit
