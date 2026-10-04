$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskEvidence=Join-Path $taskProject 'artifacts/rule-refresh-20261002'
$taskRoot=Join-Path $taskEvidence 'bundle-v4'
if(Test-Path -LiteralPath $taskRoot){throw 'Preserve existing bundle'}
New-Item -ItemType Directory -Path $taskRoot,(Join-Path $taskRoot 'bin'),(Join-Path $taskRoot 'scripts/rule-refresh') | Out-Null
Copy-Item -LiteralPath (Join-Path $taskProject 'target/release/dustaway.exe') -Destination (Join-Path $taskRoot 'dustaway.exe')
Copy-Item -LiteralPath (Join-Path $env:USERPROFILE '.cargo/bin/tauri-driver.exe') -Destination (Join-Path $taskRoot 'bin')
foreach($name in @('baseline','current')){$sourceName=if($name-eq'current'){'current-v3'}else{'baseline'};Copy-Item -LiteralPath (Join-Path $taskEvidence ('performance-'+$sourceName+'.exe')) -Destination (Join-Path $taskRoot ('bin/performance-'+$name+'.exe'))}
foreach($name in @('desktop-driver.mjs','window-state.ps1')){Copy-Item -LiteralPath (Join-Path $taskProject ('scripts/'+$name)) -Destination (Join-Path $taskRoot 'scripts')}
foreach($name in @('desktop.mjs','Compare-ScanPerformance.mjs','Run-Remote.ps1','real-files.ps1','uv-real-v3.ps1')){Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination (Join-Path $taskRoot 'scripts/rule-refresh')}
[IO.File]::WriteAllText((Join-Path $taskRoot '.dustaway-rule-refresh'),'DustAway.RuleRefresh.20261002.V4')
@{sha256=(Get-FileHash -LiteralPath (Join-Path $taskRoot 'dustaway.exe')).Hash;fixture=$false;system_cleanup=$false;version='0.18.0';build='cargo build -p dustaway --release --locked --features custom-protocol';baseline=(Get-FileHash -LiteralPath (Join-Path $taskRoot 'bin/performance-baseline.exe')).Hash;current=(Get-FileHash -LiteralPath (Join-Path $taskRoot 'bin/performance-current.exe')).Hash} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'build.json') -Encoding UTF8
Compress-Archive -Path (Join-Path $taskRoot '*') -DestinationPath (Join-Path $taskEvidence 'rule-refresh-v4-bundle.zip')
