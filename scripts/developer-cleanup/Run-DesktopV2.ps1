$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $env:USERNAME -ne 'MSN' -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-developer')) -ne 'DustAway.Developer.20261001'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Limited user required'}
if(Get-Process -Name dustaway,tauri-driver,msedgedriver -ErrorAction SilentlyContinue){throw 'Another desktop test active'}
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'build.json') -Raw | ConvertFrom-Json
$taskExe=Join-Path $taskRoot 'dustaway.exe'
if((Get-FileHash -LiteralPath $taskExe).Hash -ne $taskManifest.sha256){throw 'Wrong binary'}
$taskOutput=Join-Path $taskRoot 'evidence-desktop-v2'
if(Test-Path -LiteralPath $taskOutput){throw 'Preserve evidence'}
New-Item -ItemType Directory -Path $taskOutput | Out-Null
Copy-Item -LiteralPath (Join-Path $taskRoot 'evidence/fixtures.json') -Destination $taskOutput
$env:PATH=(Join-Path $taskRoot 'bin')+';D:\nodejs;'+$env:PATH
$env:DUSTAWAY_TEST_EXECUTABLE=$taskExe
$env:DUSTAWAY_TEST_OUTPUT=$taskOutput
$env:DUSTAWAY_UNEXECUTED_FIRST_FIXTURE='1'
Set-Location -LiteralPath $taskRoot
$taskExit=1
try {
 $taskProcess=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('scripts/developer-cleanup/desktop-v2.mjs') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskOutput 'stdout.log') -RedirectStandardError (Join-Path $taskOutput 'stderr.log')
 $taskExit=$taskProcess.ExitCode
} catch {$_ | Out-String | Set-Content -LiteralPath (Join-Path $taskOutput 'error.txt') -Encoding UTF8}
[ordered]@{exit_code=$taskExit;ordinary_user=$true;sha256=$taskManifest.sha256;finished=(Get-Date -Format o)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskOutput 'run.json') -Encoding UTF8
exit $taskExit
