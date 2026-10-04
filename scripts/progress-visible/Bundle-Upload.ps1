$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskEvidence=Join-Path $taskProject 'artifacts/progress-visible-20261002'
$taskRoot=Join-Path $taskEvidence 'bundle-v2'
if(Test-Path -LiteralPath $taskRoot){throw 'Preserve existing bundle'}
New-Item -ItemType Directory -Path $taskRoot,(Join-Path $taskRoot 'bin'),(Join-Path $taskRoot 'scripts/progress-visible') | Out-Null
Copy-Item -LiteralPath (Join-Path $taskEvidence 'dustaway-final.exe') -Destination (Join-Path $taskRoot 'dustaway.exe')
Copy-Item -LiteralPath (Join-Path $env:USERPROFILE '.cargo/bin/tauri-driver.exe') -Destination (Join-Path $taskRoot 'bin')
foreach($taskName in @('desktop-driver.mjs','window-state.ps1')){Copy-Item -LiteralPath (Join-Path $taskProject ('scripts/'+$taskName)) -Destination (Join-Path $taskRoot 'scripts')}
foreach($taskName in @('desktop.mjs','Run-Remote.ps1')){Copy-Item -LiteralPath (Join-Path $PSScriptRoot $taskName) -Destination (Join-Path $taskRoot 'scripts/progress-visible')}
[IO.File]::WriteAllText((Join-Path $taskRoot '.dustaway-progress-visible'),'DustAway.ProgressVisible.20261002.V2')
@{sha256=(Get-FileHash -LiteralPath (Join-Path $taskRoot 'dustaway.exe')).Hash;fixture=$false;system_cleanup=$false;version='0.18.0'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'build.json') -Encoding UTF8
$taskZip=Join-Path $taskEvidence 'progress-visible-v2-bundle.zip'
Compress-Archive -Path (Join-Path $taskRoot '*') -DestinationPath $taskZip
$taskOptions=@('-F','/dev/null','-i','C:/Users/Administrator/.ssh/dustaway-desktop-24njtt6-ed25519','-o','IdentitiesOnly=yes','-o','BatchMode=yes','-o','ConnectTimeout=8','-o','StrictHostKeyChecking=yes','-o','UserKnownHostsFile=C:/Users/Administrator/.ssh/dustaway-desktop-24njtt6-known-hosts','-o','GlobalKnownHostsFile=/dev/null','-o','HostKeyAlgorithms=ssh-ed25519')
& 'C:\Program Files\Git\usr\bin\scp.exe' @taskOptions $taskZip 'MSN@100.69.113.49:C:/Users/MSN/AppData/Local/DustAwayAcceptance/20260930-software/progress-visible-v2-bundle.zip'
if($LASTEXITCODE-ne0){exit $LASTEXITCODE}
& (Join-Path $taskProject 'artifacts/ssh-remote/Invoke-Remote.ps1') -ScriptFile (Join-Path $PSScriptRoot 'Prepare-Remote.ps1')
exit $LASTEXITCODE
