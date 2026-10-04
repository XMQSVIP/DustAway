$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskEvidence=Join-Path $taskProject 'artifacts/application-coverage-20261002'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$taskZip=[IO.Compression.ZipFile]::OpenRead((Join-Path $taskEvidence 'source-before.zip'))
$taskBefore=@{}
try {
 foreach($taskEntry in $taskZip.Entries){
  if($taskEntry.FullName.EndsWith('/')){continue}
  $taskStream=$taskEntry.Open();$taskHasher=[Security.Cryptography.SHA256]::Create()
  try{$taskBefore[$taskEntry.FullName]=[BitConverter]::ToString($taskHasher.ComputeHash($taskStream)).Replace('-','')}finally{$taskStream.Dispose();$taskHasher.Dispose()}
 }
}finally{$taskZip.Dispose()}
$taskPaths=@($taskBefore.Keys)
foreach($taskDirectory in @('crates','rules','src','src-tauri')){
 $taskPaths+=Get-ChildItem -LiteralPath (Join-Path $taskProject $taskDirectory) -File -Recurse | ForEach-Object {$_.FullName.Substring($taskProject.Length+1).Replace('\','/')}
}
$taskChanged=@(foreach($taskRelative in $taskPaths | Sort-Object -Unique){
 $taskPath=Join-Path $taskProject $taskRelative
 $taskAfter=if(Test-Path -LiteralPath $taskPath){(Get-FileHash -LiteralPath $taskPath).Hash}else{$null}
 if($taskBefore[$taskRelative]-ne$taskAfter){@{path=$taskRelative;before_sha256=$taskBefore[$taskRelative];after_sha256=$taskAfter}}
})
$taskChanged | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskEvidence 'source-changes-final.json') -Encoding UTF8
$taskMaterials=@(Get-ChildItem -LiteralPath $PSScriptRoot -File)
$taskMaterials+=Get-Item -LiteralPath (Join-Path $taskProject 'docs/application-coverage-audit.md'),(Join-Path $taskProject 'docs/application-coverage-evidence.md'),(Join-Path $taskProject 'docs/verification-application-coverage.md')
@($taskMaterials | ForEach-Object {@{path=$_.FullName.Substring($taskProject.Length+1).Replace('\','/');sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}}) | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $taskEvidence 'validation-materials-sha256.json') -Encoding UTF8
Write-Output "Changed source files: $($taskChanged.Count)"
