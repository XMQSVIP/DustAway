$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskEvidence=Join-Path $taskProject 'artifacts/readonly-unlock-20261002'
$taskRoot=Join-Path $taskProject 'artifacts/portable/拂尘_0.18.0_readonly-unlock-test-20261002-v1_windows-x64'
$taskNames=@('拂尘.exe','LICENSE','THIRD-PARTY-NOTICES.txt','inventory.json','portable-readme.txt','application-coverage-evidence.md','readonly-cache-evidence.md','SHA256SUMS.txt')
$taskFiles=@(Get-ChildItem -LiteralPath $taskRoot -File)
if($taskFiles.Count-ne$taskNames.Count-or@($taskFiles | Where-Object {$_.Name-notin$taskNames}).Count){throw 'Unexpected portable files'}
$taskVerified=@()
foreach($taskLine in Get-Content -LiteralPath (Join-Path $taskRoot 'SHA256SUMS.txt')){
 if($taskLine-notmatch '^([0-9A-F]{64})  (.+)$'){throw 'Malformed checksum line'}
 $taskName=$Matches[2];$taskExpected=$Matches[1]
 if($taskName-notin$taskNames){throw 'Unexpected checksum path'}
 $taskActual=(Get-FileHash -LiteralPath (Join-Path $taskRoot $taskName)).Hash
 if($taskActual-ne$taskExpected){throw "Checksum mismatch: $taskName"}
 $taskVerified+=@{name=$taskName;sha256=$taskActual}
}
if($taskVerified.Count-ne7){throw 'Missing checksums'}
$taskProgram=(Get-Content -LiteralPath (Join-Path $taskEvidence 'executable-hash.json') -Raw | ConvertFrom-Json).Hash
if((Get-FileHash -LiteralPath (Join-Path $taskRoot '拂尘.exe')).Hash-ne$taskProgram){throw 'Acceptance hash mismatch'}
Add-Type -AssemblyName System.IO.Compression.FileSystem
$taskZip=[IO.Compression.ZipFile]::OpenRead($taskRoot+'.zip')
try {
 $taskEntries=@($taskZip.Entries | Where-Object {$_.Name})
 if($taskEntries.Count-ne8-or@($taskEntries | Where-Object {$_.Name-notin$taskNames}).Count){throw 'Unexpected zip content'}
 foreach($taskEntry in $taskEntries){
  $taskStream=$taskEntry.Open();$taskHasher=[Security.Cryptography.SHA256]::Create()
  try{$taskHash=[BitConverter]::ToString($taskHasher.ComputeHash($taskStream)).Replace('-','')}finally{$taskStream.Dispose();$taskHasher.Dispose()}
  if($taskHash-ne(Get-FileHash -LiteralPath (Join-Path $taskRoot $taskEntry.Name)).Hash){throw 'Compressed content mismatch'}
 }
}finally{$taskZip.Dispose()}
@{program=$taskProgram;zip=(Get-FileHash -LiteralPath ($taskRoot+'.zip')).Hash;files=$taskVerified;no_helper_or_fixture=$true} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskEvidence 'portable-verified.json') -Encoding UTF8
