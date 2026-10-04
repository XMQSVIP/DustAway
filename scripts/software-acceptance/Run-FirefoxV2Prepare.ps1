$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath($PSScriptRoot)
if ($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or
    [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8') { throw 'Wrong dedicated environment' }
$taskPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if ($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Expected ordinary token' }
if (Get-Process -Name firefox,Un_A,FirefoxSetup,software_firefox_v2_test,dustaway -ErrorAction SilentlyContinue) { throw 'A prior browser, test or application is still active' }
$taskInstall = Join-Path $taskRoot 'firefox-real/application'
if (Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe')) { throw 'Firefox is already installed; do not replay preparation' }
$taskReady = Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-real-ready.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if ([string]$taskReady.ProfileDirectory -notmatch '^[a-zA-Z0-9_-]+\.DustAwayAcceptance20260930$') { throw 'Invalid test profile leaf' }
$taskProfile = Join-Path $env:APPDATA ('Mozilla/Firefox/Profiles/' + $taskReady.ProfileDirectory)
if ([IO.File]::ReadAllText((Join-Path $taskProfile '.dustaway-test-only')) -ne 'DustAwayFirefox20260930') { throw 'Profile marker missing' }
foreach ($taskPath in @($taskRoot, $taskProfile, $taskInstall)) {
    for ($taskPart = $taskPath; $taskPart; $taskPart = [IO.Path]::GetDirectoryName($taskPart)) {
        if (Test-Path -LiteralPath $taskPart) {
            if (((Get-Item -LiteralPath $taskPart -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Fixture path contains a link' }
        }
    }
}
foreach ($taskHive in @([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryHive]::LocalMachine)) {
    foreach ($taskView in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)) {
        $taskBase = [Microsoft.Win32.RegistryKey]::OpenBaseKey($taskHive,$taskView)
        try {
            $taskUninstall = $taskBase.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Uninstall')
            if ($taskUninstall) {
                try { foreach ($taskName in $taskUninstall.GetSubKeyNames()) {
                    $taskKey = $taskUninstall.OpenSubKey($taskName)
                    if ($taskKey) { try { if ([string]$taskKey.GetValue('DisplayName') -match '(?i)Firefox') { throw 'Existing Firefox registration; nothing installed' } } finally { $taskKey.Dispose() } }
                } } finally { $taskUninstall.Dispose() }
            }
        } finally { $taskBase.Dispose() }
    }
}
$taskInstaller = Join-Path $taskRoot 'FirefoxSetup.exe'
$taskManifest = Get-Content -LiteralPath (Join-Path $taskRoot 'installer-verification.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if ((Get-FileHash -LiteralPath $taskInstaller -Algorithm SHA256).Hash -ne $taskManifest.Sha256) { throw 'Installer hash changed' }
$taskSignature = Get-AuthenticodeSignature -LiteralPath $taskInstaller
if ($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'O=Mozilla Corporation') { throw 'Unexpected Mozilla signature' }
$taskDriverManifest = Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-v2-driver-manifest.json') -Raw -Encoding UTF8 | ConvertFrom-Json
foreach ($taskFile in $taskDriverManifest.Files.PSObject.Properties) {
    if ($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name) -Algorithm SHA256).Hash -ne $taskFile.Value) { throw 'Test driver integrity check failed' }
}
$taskOnce = [IO.File]::Open((Join-Path $taskRoot 'firefox-v2-prepare.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
[pscustomobject]@{ Stage='preparation'; Account=[Security.Principal.WindowsIdentity]::GetCurrent().Name; Elevated=$false; Session=[Diagnostics.Process]::GetCurrentProcess().SessionId; Started=(Get-Date).ToString('o'); InstallerSha256=$taskManifest.Sha256; NoPriorFirefoxRegistration=$true; ExistingMarkedTestProfileOnly=$true } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-v2-context.json') -Encoding UTF8
$taskExit = 99
try {
    $taskRun = Start-Process -FilePath $taskInstaller -ArgumentList @('/S',('/InstallDirectoryPath="'+$taskInstall+'"'),'/MaintenanceService=false','/RegisterDefaultAgent=false','/DesktopShortcut=false','/StartMenuShortcuts=false','/TaskbarShortcut=false','/PreventRebootRequired=true') -WorkingDirectory $taskRoot -WindowStyle Hidden -PassThru -Wait
    if ($taskRun.ExitCode -ne 0 -or -not (Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe'))) { throw 'Dedicated installation failed' }
    $taskBrowser = Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('firefox-profile-check.mjs','retained','firefox-v2') -WorkingDirectory $taskRoot -WindowStyle Hidden -PassThru -Wait -RedirectStandardOutput (Join-Path $taskRoot 'firefox-v2-browser-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-v2-browser-error.log')
    if ($taskBrowser.ExitCode -ne 0) { throw 'Test Cookie/local storage verification failed' }
    $taskCheck = Start-Process -FilePath (Join-Path $taskRoot 'software_firefox_v2_test.exe') -ArgumentList @('--ignored','--exact','software::firefox_scope_acceptance::real_firefox_v2_scope_preflight','--nocapture') -WorkingDirectory $taskRoot -WindowStyle Hidden -PassThru -Wait -RedirectStandardOutput (Join-Path $taskRoot 'firefox-v2-scope-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-v2-scope-error.log')
    if ($taskCheck.ExitCode -ne 0) { throw 'Independent scope preflight failed' }
    $taskExit = 0
} catch {
    $_ | Out-String | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-v2-prepare-error.log') -Encoding UTF8
}
[pscustomobject]@{ ExitCode=$taskExit; Finished=(Get-Date).ToString('o'); TestApplicationLeftInstalled=(Test-Path -LiteralPath (Join-Path $taskInstall 'firefox.exe')); UninstallExecuted=$false; ResidualDeletionExecuted=$false } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-v2-prepare-result.json') -Encoding UTF8
exit $taskExit
