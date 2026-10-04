# Bundle an already-built e2e-fixture desktop and repository tests, without installation.
param([Parameter(Mandatory=$true)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($OutputDirectory)
if(Test-Path -LiteralPath $taskRoot){throw 'Use a fresh bundle directory'}
foreach($taskDir in @('target/debug','scripts','artifacts/webdriver','bin')){
 New-Item -ItemType Directory -Path (Join-Path $taskRoot $taskDir) -Force | Out-Null
}
Copy-Item -LiteralPath 'target/debug/dustaway.exe' -Destination (Join-Path $taskRoot 'target/debug/dustaway.exe')
foreach($taskFile in @('software-desktop-check.mjs','software-exit-check.mjs','desktop-driver.mjs','window-state.ps1')){
 Copy-Item -LiteralPath (Join-Path 'scripts' $taskFile) -Destination (Join-Path $taskRoot ('scripts/'+$taskFile))
}
Copy-Item -LiteralPath 'artifacts/webdriver/msedgedriver.exe' -Destination (Join-Path $taskRoot 'artifacts/webdriver/msedgedriver.exe')
Copy-Item -LiteralPath (Join-Path $env:USERPROFILE '.cargo/bin/tauri-driver.exe') -Destination (Join-Path $taskRoot 'bin/tauri-driver.exe')
[IO.File]::WriteAllText((Join-Path $taskRoot '.dustaway-desktop-fixture'),'DustAway.Acceptance.20260930.7c7e25e8')
$taskZip=Join-Path ([IO.Path]::GetDirectoryName($taskRoot)) 'DustAwayDesktopThirdRound.zip'
if(Test-Path -LiteralPath $taskZip){throw 'Refusing to replace bundle'}
Compress-Archive -Path (Join-Path $taskRoot '*') -DestinationPath $taskZip -CompressionLevel Optimal
Get-FileHash -LiteralPath $taskZip -Algorithm SHA256
