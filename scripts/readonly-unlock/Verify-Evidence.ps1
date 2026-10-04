param([Parameter(Mandatory=$true)][ValidateSet('remote-initial-evidence','remote-v2-evidence','remote-v3-evidence','remote-final-evidence')][string]$Name)
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskEvidence=Join-Path $taskProject 'artifacts/readonly-unlock-20261002'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $taskEvidence $Name))
$taskRows=@(Get-Content -LiteralPath (Join-Path $taskRoot 'evidence-manifest.json') -Raw | ConvertFrom-Json)
if(!$taskRows.Count){throw 'Missing evidence manifest'}
foreach($taskRow in $taskRows){
 $taskPath=[IO.Path]::GetFullPath((Join-Path $taskRoot $taskRow.path))
 if(!$taskPath.StartsWith($taskRoot+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Evidence path escapes its root'}
 if((Get-FileHash -LiteralPath $taskPath).Hash-ne$taskRow.sha256){throw "Evidence hash mismatch: $($taskRow.path)"}
}
$taskBuild=Get-Content -LiteralPath (Join-Path $taskRoot 'build.json') -Raw | ConvertFrom-Json
$taskRuns=@(Get-ChildItem -LiteralPath (Join-Path $taskRoot 'evidence') -Filter run.json -Recurse -File | ForEach-Object {Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json})
foreach($taskRun in $taskRuns){if($taskRun.sha256-ne$taskBuild.sha256-or!$taskRun.ordinary_user-or$taskRun.exit_code-ne0){throw 'Run failed or identity/privilege mismatch'}}
if($taskRuns.Count-ne3-or@(@('fixture','matrix','performance') | Where-Object {$_-notin$taskRuns.phase}).Count){throw 'Missing acceptance phase'}
# New application versions are absent from the test PC; never substitute an old real-data run.
@{verified_files=$taskRows.Count;sha256=$taskBuild.sha256;runs=$taskRuns} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $taskEvidence ($Name+'-verified.json')) -Encoding UTF8
