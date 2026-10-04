$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath('artifacts/cleanup-completion-20261001/remote-bundle')
if(Test-Path -LiteralPath $taskRoot){throw 'Bundle already exists'}
foreach($taskDirectory in @('scripts/cleanup-completion','bin','artifacts/webdriver')){New-Item -ItemType Directory -Path (Join-Path $taskRoot $taskDirectory) -Force | Out-Null}
Copy-Item -LiteralPath 'target/release/dustaway.exe' -Destination (Join-Path $taskRoot 'dustaway.exe')
foreach($taskFile in @('manual-standard-check.mjs','manual-cleanup-checks.mjs','desktop-driver.mjs','window-state.ps1')){Copy-Item -LiteralPath (Join-Path 'scripts' $taskFile) -Destination (Join-Path $taskRoot ('scripts/'+$taskFile))}
foreach($taskFile in @('Run-Remote.ps1','Start-Remote.ps1','chrome-home.mjs','chrome-reopen.mjs')){Copy-Item -LiteralPath (Join-Path 'scripts/cleanup-completion' $taskFile) -Destination (Join-Path $taskRoot ('scripts/cleanup-completion/'+$taskFile))}
Copy-Item -LiteralPath (Join-Path $env:USERPROFILE '.cargo/bin/tauri-driver.exe') -Destination (Join-Path $taskRoot 'bin/tauri-driver.exe')
[IO.File]::WriteAllText((Join-Path $taskRoot '.dustaway-completion'),'DustAway.CleanupCompletion.20261001')
[ordered]@{sha256=(Get-FileHash -LiteralPath (Join-Path $taskRoot 'dustaway.exe')).Hash;version='0.18.0';fixture=$false;system_cleanup=$false;build='cargo build -p dustaway --release --locked --features custom-protocol'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'build.json') -Encoding UTF8
$taskZip=[IO.Path]::GetFullPath('artifacts/cleanup-completion-20261001/cleanup-completion-bundle.zip')
Compress-Archive -Path (Join-Path $taskRoot '*') -DestinationPath $taskZip
Get-FileHash -LiteralPath $taskZip
