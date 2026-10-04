$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskRoot=Join-Path $taskProject 'artifacts/cleanup-visible-20261001/bundle-residues'
if(Test-Path -LiteralPath $taskRoot){throw 'Bundle already exists'}
New-Item -ItemType Directory -Path $taskRoot,(Join-Path $taskRoot 'scripts/cleanup-visible'),(Join-Path $taskRoot 'bin'),(Join-Path $taskRoot 'artifacts/webdriver') | Out-Null
Copy-Item -LiteralPath (Join-Path $taskProject 'target/release/dustaway.exe') -Destination (Join-Path $taskRoot 'dustaway.exe')
foreach($taskFile in @('cleanup-visible-check.mjs','desktop-driver.mjs','window-state.ps1')){
 Copy-Item -LiteralPath (Join-Path $taskProject "scripts/$taskFile") -Destination (Join-Path $taskRoot 'scripts')
}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Run-Remote.ps1') -Destination (Join-Path $taskRoot 'scripts/cleanup-visible')
Copy-Item -LiteralPath (Join-Path $env:USERPROFILE '.cargo/bin/tauri-driver.exe') -Destination (Join-Path $taskRoot 'bin')
[IO.File]::WriteAllText((Join-Path $taskRoot '.dustaway-visible'),'DustAway.CleanupVisible.20261001')
[ordered]@{sha256=(Get-FileHash -LiteralPath (Join-Path $taskRoot 'dustaway.exe')).Hash;version='0.18.0';fixture=$false;system_cleanup=$false;build='cargo build -p dustaway --release --locked --features custom-protocol'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'build.json') -Encoding UTF8
Compress-Archive -Path (Join-Path $taskRoot '*') -DestinationPath (Join-Path $taskProject 'artifacts/cleanup-visible-20261001/cleanup-visible-residues-bundle.zip')
Get-Content -LiteralPath (Join-Path $taskRoot 'build.json')
