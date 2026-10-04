$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskEvidence=Join-Path $taskProject 'artifacts/progress-visible-20261002'
$taskSource=Join-Path $taskEvidence 'isolated-source'
$taskRoot=Join-Path $taskProject 'artifacts/portable/拂尘_0.18.0_progress-visible-test-20261002-v2_windows-x64'
if((Test-Path -LiteralPath $taskRoot)-or(Test-Path -LiteralPath ($taskRoot+'.zip'))){throw 'Preserve previous portable output'}
New-Item -ItemType Directory -Path $taskRoot | Out-Null
Copy-Item -LiteralPath (Join-Path $taskEvidence 'dustaway-final.exe') -Destination (Join-Path $taskRoot '拂尘.exe')
# Use the licenses matching the frozen dependency lock, not a concurrent build.
foreach($taskFile in @('LICENSE','licenses/THIRD-PARTY-NOTICES.txt','licenses/inventory.json','docs/application-coverage-evidence.md','docs/readonly-cache-evidence.md')){
 Copy-Item -LiteralPath (Join-Path $taskSource $taskFile) -Destination $taskRoot
}
Copy-Item -LiteralPath (Join-Path $taskProject 'docs/portable-readme.txt') -Destination $taskRoot
$taskChecksums=@(Get-ChildItem -LiteralPath $taskRoot -File | Sort-Object Name | ForEach-Object {((Get-FileHash -LiteralPath $_.FullName).Hash)+'  '+$_.Name})
Set-Content -LiteralPath (Join-Path $taskRoot 'SHA256SUMS.txt') -Value $taskChecksums -Encoding UTF8
Compress-Archive -LiteralPath $taskRoot -DestinationPath ($taskRoot+'.zip')
& (Join-Path $PSScriptRoot 'Verify-Package.ps1')
Write-Output $taskRoot
