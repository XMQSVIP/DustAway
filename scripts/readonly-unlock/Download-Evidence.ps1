$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskEvidence=Join-Path $taskProject 'artifacts/readonly-unlock-20261002'
$taskArchive=Join-Path $taskEvidence 'remote-final-evidence.zip'
$taskOutput=Join-Path $taskEvidence 'remote-final-evidence'
if((Test-Path -LiteralPath $taskArchive)-or(Test-Path -LiteralPath $taskOutput)){throw 'Preserve existing evidence'}
& (Join-Path $taskProject 'artifacts/ssh-remote/Invoke-Remote.ps1') -ScriptFile (Join-Path $PSScriptRoot 'Collect-Remote.ps1') > (Join-Path $taskEvidence 'remote-collection.json')
if($LASTEXITCODE-ne0){exit $LASTEXITCODE}
$taskRemote=Get-Content -LiteralPath (Join-Path $taskEvidence 'remote-collection.json') -Raw | ConvertFrom-Json
$taskOptions=@('-F','/dev/null','-i','C:/Users/Administrator/.ssh/dustaway-desktop-24njtt6-ed25519','-o','IdentitiesOnly=yes','-o','BatchMode=yes','-o','ConnectTimeout=8','-o','StrictHostKeyChecking=yes','-o','UserKnownHostsFile=C:/Users/Administrator/.ssh/dustaway-desktop-24njtt6-known-hosts','-o','GlobalKnownHostsFile=/dev/null','-o','HostKeyAlgorithms=ssh-ed25519')
& 'C:\Program Files\Git\usr\bin\scp.exe' @taskOptions 'MSN@100.69.113.49:C:/Users/MSN/AppData/Local/DustAwayAcceptance/20261002-readonly-unlock-v1/evidence.zip' $taskArchive
if($LASTEXITCODE-ne0){exit $LASTEXITCODE}
if((Get-FileHash -LiteralPath $taskArchive).Hash-ne$taskRemote.sha256){throw 'Evidence archive hash mismatch'}
Expand-Archive -LiteralPath $taskArchive -DestinationPath $taskOutput
& (Join-Path $PSScriptRoot 'Verify-Evidence.ps1') -Name remote-final-evidence
