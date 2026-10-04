$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskEvidence=Join-Path $taskProject 'artifacts/readonly-unlock-20261002'
$taskRoot=Join-Path $taskEvidence 'bundle-v1'
if(Test-Path -LiteralPath $taskRoot){throw 'Preserve existing bundle'}
New-Item -ItemType Directory -Path $taskRoot,(Join-Path $taskRoot 'bin'),(Join-Path $taskRoot 'scripts/readonly-unlock') | Out-Null
Copy-Item -LiteralPath (Join-Path $taskProject 'target/release/dustaway.exe') -Destination (Join-Path $taskRoot 'dustaway.exe')
Copy-Item -LiteralPath (Join-Path $env:USERPROFILE '.cargo/bin/tauri-driver.exe') -Destination (Join-Path $taskRoot 'bin')
foreach($name in @('baseline','current')){$sourceName=if($name-eq'current'){'current-delivery'}else{$name};Copy-Item -LiteralPath (Join-Path $taskEvidence ('performance-'+$sourceName+'.exe')) -Destination (Join-Path $taskRoot ('bin/performance-'+$name+'.exe'))}
foreach($name in @('desktop-driver.mjs','window-state.ps1')){Copy-Item -LiteralPath (Join-Path $taskProject ('scripts/'+$name)) -Destination (Join-Path $taskRoot 'scripts')}
foreach($name in @('desktop.mjs','Compare-ScanPerformance.mjs','Run-Remote.ps1')){Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination (Join-Path $taskRoot 'scripts/readonly-unlock')}
[IO.File]::WriteAllText((Join-Path $taskRoot '.dustaway-readonly-unlock'),'DustAway.ReadonlyUnlock.20261002.V1')
@{sha256=(Get-FileHash -LiteralPath (Join-Path $taskRoot 'dustaway.exe')).Hash;fixture=$false;system_cleanup=$false;version='0.18.0';build='cargo build -p dustaway --release --locked --features custom-protocol';baseline=(Get-FileHash -LiteralPath (Join-Path $taskRoot 'bin/performance-baseline.exe')).Hash;current=(Get-FileHash -LiteralPath (Join-Path $taskRoot 'bin/performance-current.exe')).Hash} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'build.json') -Encoding UTF8
Compress-Archive -Path (Join-Path $taskRoot '*') -DestinationPath (Join-Path $taskEvidence 'readonly-unlock-v1-bundle.zip')
