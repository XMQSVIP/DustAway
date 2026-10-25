param([Parameter(Mandatory=$true)][ValidateSet('Prepare','Start','Status','Collect','Finish','Cancel')][string]$Action,[Parameter(Mandatory=$true)][ValidatePattern('^[A-Za-z0-9-]{8,80}$')][string]$RunId)
$ErrorActionPreference='Stop'
$base='C:\Users\MSN\AppData\Local\DustAwayAcceptance\coverage-024'
$root=Join-Path $base $RunId
$taskName='DustAway-Coverage-024-'+$RunId
$marker='DustAway.Coverage.0.24.0:'+$RunId
function Assert-Directory($path){
  $current=[IO.DirectoryInfo]::new($path)
  while($null -ne $current){if($current.Exists -and ($current.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Linked directory'};$current=$current.Parent}
}
function Read-Json($name){Get-Content -LiteralPath (Join-Path $root $name) -Raw | ConvertFrom-Json}
function Get-OwnedTask {
  $task=Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
  if($task){
    $expected=Read-Json 'task.json'
    $xml=Export-ScheduledTask -TaskName $taskName
    $sha=[BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash([Text.Encoding]::UTF8.GetBytes($xml))).Replace('-','')
    if($sha -cne $expected.sha256){throw 'Task ownership changed'}
  }
  return $task
}
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong test host'}
Assert-Directory $root
if($Action -eq 'Prepare'){
  if(Test-Path -LiteralPath $root){throw 'Run already exists; query status instead'}
  New-Item -ItemType Directory -Path $root -Force | Out-Null
  [IO.File]::WriteAllText((Join-Path $root '.dustaway-test'),$marker)
  [pscustomobject]@{runId=$RunId;status='prepared'}|ConvertTo-Json -Compress
  return
}
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test')) -cne $marker){throw 'Wrong run marker'}
if($Action -eq 'Start'){
  if((Test-Path -LiteralPath (Join-Path $root 'started.json')) -or (Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue)){throw 'Run cannot be replayed'}
  $request=Read-Json 'request.json'
  if($request.schema -ne 3 -or $request.runId -cne $RunId -or $request.version -ne '0.24.0'){throw 'Wrong request'}
  foreach($name in @('worker.ps1','windows-tests.exe','core-tests.exe','probe.exe')){
    $file=Get-Item -LiteralPath (Join-Path $root $name)
    if($file.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Linked input'}
    if($request.hashes.$name -notmatch '^[A-Fa-f0-9]{64}$' -or (Get-FileHash -LiteralPath $file.FullName).Hash -ine $request.hashes.$name){throw 'Input hash mismatch'}
  }
  $ordinarySession=$false
  foreach($process in Get-CimInstance Win32_Process -Filter "Name='explorer.exe'"){
    $owner=Invoke-CimMethod -InputObject $process -MethodName GetOwner
    if($owner.ReturnValue -eq 0 -and $owner.User -ieq 'MSN' -and $process.SessionId -gt 0){$ordinarySession=$true}
  }
  $dispatch=[IO.File]::Open((Join-Path $root 'started.json'),[IO.FileMode]::CreateNew)
  try{$bytes=[Text.Encoding]::UTF8.GetBytes((@{runId=$RunId;requestSha256=(Get-FileHash (Join-Path $root 'request.json')).Hash}|ConvertTo-Json));$dispatch.Write($bytes,0,$bytes.Length);$dispatch.Flush($true)}finally{$dispatch.Dispose()}
  if(-not $ordinarySession){
    @{schema=3;runId=$RunId;host=$env:COMPUTERNAME;ordinaryUser=$false;status='blocked';stages=@();realDeletionAttempted=$false;webpageRecoveryVerified=$false}|ConvertTo-Json -Depth 8|Set-Content (Join-Path $root 'status.json') -Encoding utf8
  }else{
    $worker=Join-Path $root 'worker.ps1'
    $expected=$request.hashes.'worker.ps1'
    $command="`$ErrorActionPreference='Stop'; if((Get-FileHash -LiteralPath '$worker').Hash -ine '$expected'){throw 'Worker changed'}; & ([ScriptBlock]::Create([IO.File]::ReadAllText('$worker'))) -RunRoot '$root'"
    $encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
    $taskAction=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand '+$encoded) -WorkingDirectory $root
    $principal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
    Register-ScheduledTask -TaskName $taskName -Action $taskAction -Principal $principal -Settings (New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::FromMinutes(42))) | Out-Null
    $xml=Export-ScheduledTask -TaskName $taskName
    $hash=[BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash([Text.Encoding]::UTF8.GetBytes($xml))).Replace('-','')
    @{sha256=$hash}|ConvertTo-Json|Set-Content (Join-Path $root 'task.json') -Encoding utf8
    Start-ScheduledTask -TaskName $taskName
  }
  @{runId=$RunId;status='dispatched'}|ConvertTo-Json -Compress
  return
}
$task=Get-OwnedTask
if($Action -eq 'Cancel'){
  [IO.File]::WriteAllText((Join-Path $root 'cancel.request'),'cancel requested')
  @{runId=$RunId;status='stopping'}|ConvertTo-Json -Compress
  return
}
$status=if(Test-Path -LiteralPath (Join-Path $root 'status.json')){Read-Json 'status.json'}else{@{schema=3;runId=$RunId;host=$env:COMPUTERNAME;ordinaryUser=$false;status='uncertain';stages=@();realDeletionAttempted=$false;webpageRecoveryVerified=$false}}
if($task -and $task.State -eq 'Running'){
  if($Action -in @('Collect','Finish')){throw 'Task has not ended'}
  $status.status='running'
}elseif($status.status -eq 'running'){
  $status.status='uncertain'
  $status|ConvertTo-Json -Depth 20|Set-Content (Join-Path $root 'status.json') -Encoding utf8
}
if($Action -eq 'Finish'){
  if($task){Unregister-ScheduledTask -TaskName $taskName -Confirm:$false}
  @{runId=$RunId;status='finished';evidenceRetained=$true}|ConvertTo-Json -Compress
}elseif($Action -eq 'Collect'){
  $started=Read-Json 'started.json'
  if((Get-FileHash (Join-Path $root 'request.json')).Hash -cne $started.requestSha256){throw 'Run request changed'}
  $names=@('request.json','status.json','worker.ps1','windows-tests.exe','core-tests.exe','probe.exe','started.json','task.json','worker.err')
  foreach($stage in $status.stages){foreach($suffix in @('.log','.err','.intent.json')){$names+=($stage.id+$suffix)}}
  $files=@(foreach($name in $names){$path=Join-Path $root $name;if(Test-Path -LiteralPath $path){$file=Get-Item -LiteralPath $path;if($file.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Linked evidence'};@{name=$name;bytes=$file.Length;sha256=(Get-FileHash -LiteralPath $path).Hash.ToLower()}}})
  $manifest=@{schema=3;runId=$RunId;mode='noninteractive';files=$files}
  $manifest|ConvertTo-Json -Depth 20|Set-Content (Join-Path $root 'manifest.json') -Encoding utf8
  $manifest|ConvertTo-Json -Depth 20 -Compress
}else{$status|ConvertTo-Json -Depth 20 -Compress}
