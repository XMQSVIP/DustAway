$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskZip=Join-Path $taskProject 'artifacts/application-coverage-20261002/application-coverage-v1-bundle.zip'
if(!(Test-Path -LiteralPath $taskZip)){throw 'Build the verified bundle first'}
$taskOptions=@('-F','/dev/null','-i','C:/Users/Administrator/.ssh/dustaway-desktop-24njtt6-ed25519','-o','IdentitiesOnly=yes','-o','BatchMode=yes','-o','ConnectTimeout=8','-o','StrictHostKeyChecking=yes','-o','UserKnownHostsFile=C:/Users/Administrator/.ssh/dustaway-desktop-24njtt6-known-hosts','-o','GlobalKnownHostsFile=/dev/null','-o','HostKeyAlgorithms=ssh-ed25519')
& 'C:\Program Files\Git\usr\bin\scp.exe' @taskOptions $taskZip 'MSN@100.69.113.49:C:/Users/MSN/AppData/Local/DustAwayAcceptance/20260930-software/application-coverage-v1-bundle.zip'
if($LASTEXITCODE-ne0){exit $LASTEXITCODE}
& (Join-Path $taskProject 'artifacts/ssh-remote/Invoke-Remote.ps1') -ScriptFile (Join-Path $PSScriptRoot 'Prepare-Remote.ps1')
exit $LASTEXITCODE
