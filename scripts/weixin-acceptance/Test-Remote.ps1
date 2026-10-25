param([Parameter(Mandatory=$true)][string]$RemoteSourceBase64)
$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$script=[ScriptBlock]::Create([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($RemoteSourceBase64)))
$id='negative-'+[Guid]::NewGuid().ToString('N')
$root='C:\Users\MSN\AppData\Local\DustAwayAcceptance\weixin-0211\'+$id
$name='DustAway-Weixin-0211-'+$id
& $script -Action Prepare -RunId $id | Out-Null
$duplicate=$false
try{& $script -Action Prepare -RunId $id | Out-Null}catch{$duplicate=$true}
if(-not $duplicate){throw 'Duplicate accepted'}
$hashes=@{}
foreach($file in @('worker.ps1','weixin-tests.exe','core-tests.exe','probe.exe')){[IO.File]::WriteAllText((Join-Path $root $file),'not executable synthetic input');$hashes[$file]='0'*64}
@{schema=2;runId=$id;version='0.21.1';hashes=$hashes}|ConvertTo-Json -Depth 8|Set-Content (Join-Path $root 'request.json') -Encoding utf8
$changed=$false
try{& $script -Action Start -RunId $id | Out-Null}catch{$changed=$true}
if(-not $changed -or (Test-Path (Join-Path $root 'started.json'))){throw 'Changed binary accepted'}
$action=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument '-NoProfile -NonInteractive -Command exit' -WorkingDirectory $root
$principal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $name -Action $action -Principal $principal | Out-Null
$ownedXml=Export-ScheduledTask -TaskName $name
try{
  @{sha256=('0'*64)}|ConvertTo-Json|Set-Content (Join-Path $root 'task.json') -Encoding utf8
  $foreign=$false
  try{& $script -Action Finish -RunId $id | Out-Null}catch{$foreign=$true}
  if(-not $foreign -or -not (Get-ScheduledTask -TaskName $name)){throw 'Foreign task removed'}
  @{schema=2;runId=$id;duplicateRunRejected=$duplicate;changedBinaryRejected=$changed;foreignTaskPreserved=$foreign}|ConvertTo-Json
}finally{
  if((Export-ScheduledTask -TaskName $name) -cne $ownedXml){throw 'Negative fixture task changed'}
  Unregister-ScheduledTask -TaskName $name -Confirm:$false
}
