param([string]$TestRoot)
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($TestRoot)
if($env:COMPUTERNAME-ne'DESKTOP-24NJTT6'-or$env:USERNAME-ne'MSN'-or[IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-progress-visible'))-ne'DustAway.ProgressVisible.20261002.V2'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Limited user required'}
if(Get-Process -Name dustaway,tauri-driver,msedgedriver -ErrorAction SilentlyContinue){throw 'Another desktop test is active'}
$taskExe=Join-Path $taskRoot 'dustaway.exe'
$taskBuild=Get-Content -LiteralPath (Join-Path $taskRoot 'build.json') -Raw | ConvertFrom-Json
if((Get-FileHash -LiteralPath $taskExe).Hash-ne$taskBuild.sha256){throw 'Wrong program'}
$taskOutput=Join-Path $taskRoot 'evidence'
if(Test-Path -LiteralPath $taskOutput){throw 'Preserve evidence; do not replay'}
New-Item -ItemType Directory -Path $taskOutput | Out-Null
$env:DUSTAWAY_TEST_OUTPUT=$taskOutput
$env:DUSTAWAY_TEST_EXECUTABLE=$taskExe
$env:PATH=(Join-Path $taskRoot 'bin')+';D:\nodejs;'+$env:PATH
$taskExit=1
try{
 $taskProcess=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList 'scripts/progress-visible/desktop.mjs' -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskOutput 'stdout.log') -RedirectStandardError (Join-Path $taskOutput 'stderr.log')
 $taskExit=$taskProcess.ExitCode
}catch{$_ | Out-String | Set-Content -LiteralPath (Join-Path $taskOutput 'error.txt') -Encoding UTF8}
@{exit_code=$taskExit;ordinary_user=$true;sha256=$taskBuild.sha256;finished=(Get-Date -Format o)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskOutput 'run.json') -Encoding UTF8
exit $taskExit
