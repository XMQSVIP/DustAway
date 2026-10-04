param([ValidateSet('matrix','real','chrome','uv')][string]$Phase='matrix',[string]$TestRoot)
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($TestRoot)
if($env:COMPUTERNAME-ne'DESKTOP-24NJTT6'-or$env:USERNAME-ne'MSN'-or[IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-space-relief'))-ne'DustAway.SpaceRelief.20261002'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Limited user required'}
if(Get-Process -Name dustaway,tauri-driver,msedgedriver -ErrorAction SilentlyContinue){throw 'Another desktop test is active'}
$taskExe=Join-Path $taskRoot 'dustaway.exe'
$manifest=Get-Content -LiteralPath (Join-Path $taskRoot 'build.json') -Raw | ConvertFrom-Json
if((Get-FileHash -LiteralPath $taskExe).Hash-ne$manifest.sha256){throw 'Wrong build'}
$taskOutput=Join-Path $taskRoot ('evidence/'+$Phase)
if(Test-Path -LiteralPath $taskOutput){throw 'Never overwrite evidence or replay a requested cleanup'}
New-Item -ItemType Directory -Path $taskOutput | Out-Null
$env:DUSTAWAY_TEST_OUTPUT=$taskOutput
$env:DUSTAWAY_TEST_EXECUTABLE=$taskExe
$env:PATH=(Join-Path $taskRoot 'bin')+';D:\nodejs;'+$env:PATH
Set-Location -LiteralPath $taskRoot
$taskExit=1
try {
 if($Phase-eq'matrix'){
  $env:UV_CACHE_DIR=Join-Path $taskRoot 'uv-fixture-cache'
  $taskProcess=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList 'scripts/space-relief/desktop.mjs' -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskOutput 'stdout.log') -RedirectStandardError (Join-Path $taskOutput 'stderr.log')
  if($taskProcess.ExitCode-ne 0){throw 'Matrix failed; inspect before any retry'}
 } elseif($Phase-eq'real'){
  if(Test-Path Env:UV_CACHE_DIR){Remove-Item Env:UV_CACHE_DIR}
  & ([scriptblock]::Create([IO.File]::ReadAllText((Join-Path $taskRoot 'scripts/space-relief/real-cache-v2.ps1')))) -TestRoot $taskRoot
 } elseif($Phase-eq'uv'){
  if(Test-Path Env:UV_CACHE_DIR){Remove-Item Env:UV_CACHE_DIR}
  & ([scriptblock]::Create([IO.File]::ReadAllText((Join-Path $taskRoot 'scripts/space-relief/uv-real.ps1')))) -TestRoot $taskRoot
 } else {
  foreach($s in @('chrome-home.mjs','chrome-reopen.mjs')){
   $taskProcess=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList ('scripts/cleanup-completion/'+$s) -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskOutput ($s+'.log')) -RedirectStandardError (Join-Path $taskOutput ($s+'.error.log'))
   if($taskProcess.ExitCode-ne 0){throw "Chrome check failed: $s"}
  }
 }
 $taskExit=0
} catch {$_ | Out-String | Set-Content -LiteralPath (Join-Path $taskOutput 'error.txt') -Encoding UTF8}
@{phase=$Phase;exit_code=$taskExit;ordinary_user=$true;sha256=$manifest.sha256;finished=(Get-Date -Format o)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskOutput 'run.json') -Encoding UTF8
exit $taskExit
