$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $env:USERNAME -ne 'MSN' -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-developer')) -ne 'DustAway.Mainstream.20261001'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Limited user required'}
if(Get-Process -Name dustaway,tauri-driver,msedgedriver -ErrorAction SilentlyContinue){throw 'Another desktop test is active'}
if(Get-NetTCPConnection -LocalPort 4444,4445 -State Listen -ErrorAction SilentlyContinue){throw 'Driver port occupied'}
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'build.json') -Raw | ConvertFrom-Json
$taskExe=Join-Path $taskRoot 'dustaway.exe'
if((Get-FileHash -LiteralPath $taskExe).Hash -ne $taskManifest.sha256 -or $taskManifest.fixture -ne $false -or $taskManifest.system_cleanup -ne $false){throw 'Wrong build'}
$taskOutput=Join-Path $taskRoot 'evidence'
if(Test-Path -LiteralPath $taskOutput){throw 'Evidence already exists'}
New-Item -ItemType Directory -Path $taskOutput | Out-Null
$env:PATH=(Join-Path $taskRoot 'bin')+';D:\nodejs;'+$env:PATH
$env:DUSTAWAY_TEST_EXECUTABLE=$taskExe
$env:DUSTAWAY_TEST_OUTPUT=$taskOutput
Set-Location -LiteralPath $taskRoot
$taskExit=1
try {
 $taskHarness=Join-Path $taskRoot 'bin/mainstream_fixture.exe'
 if((Get-FileHash -LiteralPath $taskHarness).Hash -ne $taskManifest.harness_sha256){throw 'Wrong fixture harness'}
 foreach($taskMode in @('seed','clean')){
  $taskFixture=Start-Process -FilePath $taskHarness -ArgumentList @($taskMode) -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskOutput "fixture-$taskMode-stdout.log") -RedirectStandardError (Join-Path $taskOutput "fixture-$taskMode-stderr.log")
  if($taskFixture.ExitCode -ne 0){throw "Fixture $taskMode failed; do not replay deletion"}
 }
 $env:UV_CACHE_DIR=Join-Path $taskRoot 'uv-fixture-cache'
 $taskProcess=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('scripts/developer-mainstream/desktop.mjs') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskOutput 'stdout.log') -RedirectStandardError (Join-Path $taskOutput 'stderr.log')
 $taskExit=$taskProcess.ExitCode
} catch {$_ | Out-String | Set-Content -LiteralPath (Join-Path $taskOutput 'error.txt') -Encoding UTF8}
[ordered]@{exit_code=$taskExit;ordinary_user=$true;sha256=$taskManifest.sha256;finished=(Get-Date -Format o)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskOutput 'run.json') -Encoding UTF8
exit $taskExit
