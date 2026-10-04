$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskRoot=Join-Path $taskProject 'artifacts/developer-cleanup-20261001'
$taskRemote=Join-Path $taskRoot 'remote-evidence'
$taskBuild=Get-Content -LiteralPath (Join-Path $taskRemote 'build.json') -Raw | ConvertFrom-Json
$taskRun=Get-Content -LiteralPath (Join-Path $taskRemote 'evidence-desktop-v2/run.json') -Raw | ConvertFrom-Json
$taskFixture=Get-Content -LiteralPath (Join-Path $taskRemote 'evidence/fixture-success.json') -Raw | ConvertFrom-Json
$taskDesktop=Get-Content -LiteralPath (Join-Path $taskRemote 'evidence-desktop-v2/desktop-results.json') -Raw | ConvertFrom-Json
if($taskRun.exit_code -ne 0 -or -not $taskRun.ordinary_user -or -not $taskFixture.passed -or -not $taskDesktop.passed -or $taskDesktop.results.Count -ne 4){throw 'Incomplete remote acceptance'}
$taskEvidence=Get-Content -LiteralPath (Join-Path $taskRemote 'evidence-manifest.json') -Raw | ConvertFrom-Json
foreach($taskEntry in $taskEvidence){
 $taskPath=[IO.Path]::GetFullPath((Join-Path $taskRemote $taskEntry.path))
 if(-not $taskPath.StartsWith($taskRemote+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Manifest outside evidence root'}
 if((Get-Item -LiteralPath $taskPath).Length -ne $taskEntry.bytes -or (Get-FileHash -LiteralPath $taskPath).Hash -ne $taskEntry.sha256){throw 'Evidence mismatch'}
}
$taskPackage=Join-Path $taskProject 'artifacts/portable/拂尘_0.18.0_developer-cache-test-20261001_windows-x64'
$taskFiles=@(Get-ChildItem -LiteralPath $taskPackage -File)
$taskExpected=@('拂尘.exe','LICENSE','THIRD-PARTY-NOTICES.txt','inventory.json','portable-readme.txt','SHA256SUMS.txt')
if((Compare-Object ($taskFiles.Name | Sort-Object) ($taskExpected | Sort-Object)) -or (Get-ChildItem -LiteralPath $taskPackage -Directory)){throw 'Unexpected package entries'}
$taskExeHash=(Get-FileHash -LiteralPath (Join-Path $taskPackage '拂尘.exe')).Hash
if($taskExeHash -ne $taskBuild.sha256 -or $taskRun.sha256 -ne $taskExeHash){throw 'Program differs from acceptance'}
foreach($taskResult in $taskDesktop.results){if($taskResult.binarySha256 -ne $taskExeHash){throw 'Desktop hash differs'}}
foreach($taskLine in Get-Content -LiteralPath (Join-Path $taskPackage 'SHA256SUMS.txt')){
 if($taskLine -notmatch '^([A-Fa-f0-9]{64})  (.+)$'){throw 'Invalid checksum line'}
 if((Get-FileHash -LiteralPath (Join-Path $taskPackage $Matches[2])).Hash -ne $Matches[1]){throw 'Package checksum differs'}
}
$taskZip=$taskPackage+'.zip'
if(Test-Path -LiteralPath $taskZip){throw 'Preserve previous package'}
Compress-Archive -LiteralPath $taskPackage -DestinationPath $taskZip
$taskRecord=[ordered]@{passed=$true;exe_sha256=$taskExeHash;zip=$taskZip;zip_sha256=(Get-FileHash -LiteralPath $taskZip).Hash;evidence_files=$taskEvidence.Count;backend_fixture_deleted=$taskFixture.deleted;desktop_flows=$taskDesktop.results.Count;package_files=$taskFiles.Name}
$taskRecord | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskRoot 'delivery-verification.json') -Encoding UTF8
$taskRecord | ConvertTo-Json -Depth 4
