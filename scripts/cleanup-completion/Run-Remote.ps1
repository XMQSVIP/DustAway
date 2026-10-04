param([ValidateSet('native','chrome')][string]$Phase='native', [ValidatePattern('^[a-z0-9-]+$')][string]$Run='first')
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $env:USERNAME -ne 'MSN' -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-completion')) -ne 'DustAway.CleanupCompletion.20261001'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
if(Get-Process -Name dustaway,tauri-driver,msedgedriver -ErrorAction SilentlyContinue){throw 'Another desktop test is active'}
if(Get-NetTCPConnection -LocalPort 4444,4445 -State Listen -ErrorAction SilentlyContinue){throw 'Driver port occupied'}
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'build.json') -Raw | ConvertFrom-Json
$taskExe=Join-Path $taskRoot 'dustaway.exe'
if((Get-FileHash -LiteralPath $taskExe).Hash -ne $taskManifest.sha256 -or $taskManifest.fixture -ne $false -or $taskManifest.system_cleanup -ne $false){throw 'Wrong build'}
$taskOutput=Join-Path $taskRoot "evidence/$Phase-$Run"
if(Test-Path -LiteralPath $taskOutput){throw 'Use a fresh evidence run'}
New-Item -ItemType Directory -Path $taskOutput | Out-Null
$env:PATH=(Join-Path $taskRoot 'bin')+';D:\nodejs;'+$env:PATH
$env:DUSTAWAY_TEST_EXECUTABLE=$taskExe
$env:DUSTAWAY_TEST_OUTPUT=$taskOutput
Set-Location -LiteralPath $taskRoot
function Run-TestNode([string]$Script,[string]$Log){
 $taskProcess=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @($Script) -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput ($Log+'.log') -RedirectStandardError ($Log+'.error.log')
 if($taskProcess.ExitCode -ne 0){throw "Test failed: $Script ($($taskProcess.ExitCode))"}
}
$taskExit=1
try {
 if($Phase -eq 'native'){
  foreach($taskTheme in @('light','dark')){
   foreach($taskWidth in @(760,1100)){
    $env:DUSTAWAY_TEST_THEME=$taskTheme
    $env:DUSTAWAY_TEST_WIDTH=[string]$taskWidth
    $env:DUSTAWAY_TEST_OUTPUT=Join-Path $taskOutput "$taskTheme-$taskWidth"
    Run-TestNode 'scripts/manual-standard-check.mjs' (Join-Path $taskOutput "$taskTheme-$taskWidth")
   }
  }
 } else {
  Run-TestNode 'scripts/cleanup-completion/chrome-home.mjs' (Join-Path $taskOutput 'chrome')
  Run-TestNode 'scripts/cleanup-completion/chrome-reopen.mjs' (Join-Path $taskOutput 'chrome-reopen')
 }
 $taskExit=0
} catch {$_ | Out-String | Set-Content -LiteralPath (Join-Path $taskOutput 'error.txt') -Encoding UTF8}
[ordered]@{phase=$Phase;exit_code=$taskExit;ordinary_user=$true;sha256=$taskManifest.sha256;finished=(Get-Date -Format o)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskOutput 'result.json') -Encoding UTF8
exit $taskExit
