param([Parameter(Mandatory = $true)][string]$File)
$ErrorActionPreference = 'Stop'
if (-not $env:DUSTAWAY_CERT_THUMBPRINT) { throw 'DUSTAWAY_CERT_THUMBPRINT is required for a signed release' }
$taskFile = (Resolve-Path -LiteralPath $File).Path
$taskSignTool = (Get-Command signtool.exe -ErrorAction Stop).Source
& $taskSignTool sign /sha1 $env:DUSTAWAY_CERT_THUMBPRINT /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $taskFile
if ($LASTEXITCODE -ne 0) { throw "Signing failed: $taskFile" }
& $taskSignTool verify /pa $taskFile
if ($LASTEXITCODE -ne 0) { throw "Signature verification failed: $taskFile" }
