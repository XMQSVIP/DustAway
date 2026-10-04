$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskEvidence=Join-Path $taskProject 'artifacts/rule-refresh-20261002'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$taskZip=[IO.Compression.ZipFile]::OpenRead((Join-Path $taskEvidence 'source-before.zip'))
try {
 $taskChanged=@()
 $taskConfig=@(Get-Content -LiteralPath (Join-Path $taskEvidence 'performance-matched-config.json') -Raw | ConvertFrom-Json)
 if($taskConfig.Count-ne6){throw 'Missing baseline build configuration evidence'}
 foreach($taskRow in $taskConfig){$taskRow.current=(Get-FileHash -LiteralPath (Join-Path $taskProject $taskRow.path)).Hash}
 foreach($taskEntry in $taskZip.Entries){
  if($taskEntry.FullName.EndsWith('/')){continue}
  $taskPath=Join-Path $taskProject $taskEntry.FullName
  $taskStream=$taskEntry.Open()
  $taskHasher=[Security.Cryptography.SHA256]::Create()
  try{$taskBefore=[BitConverter]::ToString($taskHasher.ComputeHash($taskStream)).Replace('-','')}finally{$taskStream.Dispose();$taskHasher.Dispose()}
  $taskAfter=if(Test-Path -LiteralPath $taskPath){(Get-FileHash -LiteralPath $taskPath).Hash}else{$null}
  if($taskBefore-ne$taskAfter){$taskChanged+=@{path=$taskEntry.FullName;before=$taskBefore;after=$taskAfter}}
 }
 if(@($taskConfig | Where-Object {$_.baseline-ne$_.current}).Count){throw 'Benchmark build configuration changed'}
 @{changed=$taskChanged;configuration=$taskConfig} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $taskEvidence 'source-comparison.json') -Encoding UTF8
}finally{$taskZip.Dispose()}
