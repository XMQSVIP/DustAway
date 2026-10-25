# Local compatibility entry: query the existing SSH run, never scan via PowerShell.
param([Parameter(Mandatory=$true)][ValidatePattern('^[A-Za-z0-9-]{8,80}$')][string]$RunId)
$ErrorActionPreference='Stop'
& node (Join-Path $PSScriptRoot 'run.mjs') status $RunId
exit $LASTEXITCODE
