$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskRoot=Join-Path $taskProject 'artifacts/space-relief-20261002/bundle'
if(Test-Path -LiteralPath $taskRoot){throw 'Preserve existing bundle'}
New-Item -ItemType Directory -Path $taskRoot,(Join-Path $taskRoot 'bin'),(Join-Path $taskRoot 'scripts/space-relief'),(Join-Path $taskRoot 'scripts/cleanup-completion') | Out-Null
Copy-Item -LiteralPath (Join-Path $taskProject 'target/release/dustaway.exe') -Destination (Join-Path $taskRoot 'dustaway.exe')
Copy-Item -LiteralPath (Join-Path $env:USERPROFILE '.cargo/bin/tauri-driver.exe') -Destination (Join-Path $taskRoot 'bin')
foreach($s in @('desktop-driver.mjs','window-state.ps1')){Copy-Item -LiteralPath (Join-Path $taskProject ('scripts/'+$s)) -Destination (Join-Path $taskRoot 'scripts')}
foreach($s in @('desktop.mjs','real-files.ps1','uv-real-v3.ps1','Run-Remote.ps1')){Copy-Item -LiteralPath (Join-Path $PSScriptRoot $s) -Destination (Join-Path $taskRoot 'scripts/space-relief')}
foreach($s in @('chrome-home.mjs','chrome-reopen.mjs')){Copy-Item -LiteralPath (Join-Path $taskProject ('scripts/cleanup-completion/'+$s)) -Destination (Join-Path $taskRoot 'scripts/cleanup-completion')}
[IO.File]::WriteAllText((Join-Path $taskRoot '.dustaway-space-relief'),'DustAway.SpaceRelief.20261002')
[IO.File]::WriteAllText((Join-Path $taskRoot '.dustaway-completion'),'DustAway.CleanupCompletion.20261001')
@{sha256=(Get-FileHash -LiteralPath (Join-Path $taskRoot 'dustaway.exe')).Hash;fixture=$false;system_cleanup=$false;version='0.18.0';build='cargo build -p dustaway --release --locked --features custom-protocol'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'build.json') -Encoding UTF8
Compress-Archive -Path (Join-Path $taskRoot '*') -DestinationPath (Join-Path $taskProject 'artifacts/space-relief-20261002/space-relief-bundle.zip')
