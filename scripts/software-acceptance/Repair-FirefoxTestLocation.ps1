$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
$taskBase=[IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance'))
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $taskBase '20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
if(Get-Process -Name firefox,Un_A,Au_,FirefoxSetup,software_firefox_production_test -ErrorAction SilentlyContinue){throw 'Prior vendor process remains'}
$taskResult=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-production-result.json') -Raw | ConvertFrom-Json
if($taskResult.ExitCode -ne 101 -or [IO.File]::ReadAllText((Join-Path $taskRoot 'firefox-production-error.log')).Trim() -ne 'Error: reinstallation not verified'){throw 'Unexpected previous failure'}
$taskKey='HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Mozilla Firefox 140.17.0 ESR (x64 en-US)'
$taskRegistration=Get-ItemProperty -LiteralPath $taskKey
$taskHelper=Join-Path $taskBase 'uninstall\helper.exe'
if([IO.Path]::GetFullPath($taskRegistration.InstallLocation).TrimEnd('\') -ne $taskBase -or $taskRegistration.UninstallString -ne ('"'+$taskHelper+'"')){throw 'Unexpected test registration'}
foreach($taskExe in @($taskHelper,(Join-Path $taskBase 'firefox.exe'))) {
 $taskSignature=Get-AuthenticodeSignature -LiteralPath $taskExe
 if($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'O=Mozilla Corporation'){throw 'Test executable signature mismatch'}
}
$taskVersion=[Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $taskBase 'firefox.exe'))
if((@($taskVersion.FileMajorPart,$taskVersion.FileMinorPart,$taskVersion.FileBuildPart,$taskVersion.FilePrivatePart)-join '.') -ne '140.17.0.1158'){throw 'Test executable version mismatch'}
$taskFiles=@(Get-ChildItem -LiteralPath $taskRoot -File -Recurse)
if(@($taskFiles | Where-Object {$_.Attributes -band [IO.FileAttributes]::ReparsePoint}).Count){throw 'Linked evidence file'}
$taskBefore=[ordered]@{}
foreach($taskFile in $taskFiles){$taskBefore[$taskFile.FullName]=(Get-FileHash -LiteralPath $taskFile.FullName -Algorithm SHA256).Hash}
$taskOnce=[IO.File]::Open((Join-Path $taskRoot 'firefox-location-repair.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
$taskRun=Start-Process -FilePath $taskHelper -ArgumentList '/S' -WorkingDirectory $taskBase -WindowStyle Hidden -Wait -PassThru
if($taskRun.ExitCode -ne 0 -or (Test-Path -LiteralPath $taskKey) -or (Test-Path -LiteralPath (Join-Path $taskBase 'firefox.exe'))){throw 'Test installation removal not verified'}
foreach($taskFile in $taskBefore.Keys){if(-not(Test-Path -LiteralPath $taskFile) -or (Get-FileHash -LiteralPath $taskFile -Algorithm SHA256).Hash -ne $taskBefore[$taskFile]){throw 'A test evidence file changed'}}
[pscustomobject]@{Passed=$true;Finished=(Get-Date).ToString('o');OrdinaryUser=$true;MisplacedTestInstallationRemoved=$true;EvidenceFilesUnchanged=$taskBefore.Count;RecursiveDeletion=$false;ProfileHashesNotMeasured=$true} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-location-repair-result.json') -Encoding UTF8
