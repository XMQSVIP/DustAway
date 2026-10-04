param([string]$TestRoot)
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($TestRoot)
if($env:COMPUTERNAME-ne'DESKTOP-24NJTT6'-or$env:USERNAME-ne'MSN'-or[IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-space-relief'))-ne'DustAway.SpaceRelief.20261002'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Limited user required'}
$taskOutput=Join-Path $taskRoot 'evidence/uv-postcheck2'
if(Test-Path -LiteralPath $taskOutput){throw 'Never overwrite evidence'}
New-Item -ItemType Directory -Path $taskOutput | Out-Null
try {
 $manifest=[IO.File]::ReadAllText((Join-Path $taskRoot 'build.json')) | ConvertFrom-Json
 $hash=(Get-FileHash -LiteralPath (Join-Path $taskRoot 'dustaway.exe')).Hash
 if($hash-ne$manifest.sha256){throw 'Wrong build'}
 $report=[IO.File]::ReadAllText((Join-Path $taskRoot 'evidence/uv2/uv-real-report.json')) | ConvertFrom-Json
 if($report.status-ne'completed'){throw 'Original uv command did not complete; never replay'}
 $protected=[IO.File]::ReadAllText((Join-Path $taskRoot 'evidence/uv2/uv-protected.json')) | ConvertFrom-Json
 foreach($r in $protected){
  if($r.kind-eq'link'){
   $i=Get-Item -LiteralPath $r.path -Force
   if([string]$i.Target-ne$r.target-or[string]$i.LinkType-ne$r.link_type-or[string]$i.Attributes-ne$r.attributes-or$i.LastWriteTimeUtc.Ticks-ne$r.modified){throw "Protected link changed: $($r.path)"}
  } elseif(!(Test-Path -LiteralPath $r.path)-or(Get-FileHash -LiteralPath $r.path).Hash-ne$r.sha256){throw "Protected file changed: $($r.path)"}
 }
 $testEnvironment=Join-Path $taskRoot 'evidence/uv2/protected-project-env'
 # No embedded quoted string in the native -c argument (Windows PowerShell 5).
 & (Join-Path $testEnvironment 'Scripts/python.exe') -c 'import sys; assert sys.prefix != sys.base_prefix; print(sys.prefix)' *> (Join-Path $taskOutput 'uv-environment-check.log')
 if($LASTEXITCODE-ne 0){throw 'Protected Python environment check failed'}
 & 'D:\Python312\python.exe' -m pip --version *> (Join-Path $taskOutput 'pip-version.log')
 if($LASTEXITCODE-ne 0){throw 'pip startup check failed'}
 & 'D:\nodejs\node.exe' 'D:\nodejs\node_modules\npm\bin\npm-cli.js' --version *> (Join-Path $taskOutput 'npm-version.log')
 if($LASTEXITCODE-ne 0){throw 'npm startup check failed'}
 @{sha256=$hash;ordinary_user=$true;protected_unchanged=$true;protected_count=$protected.Count;protected_links=@($protected | Where-Object {$_.kind-eq'link'}).Count;uv_report='uv2/uv-real-report.json';uv_task_id=$report.id;status=$report.status;no_cleanup_resubmitted=$true;python_environment_works=$true;pip_npm_startup=$true;finished=(Get-Date -Format o)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskOutput 'result.json') -Encoding UTF8
} catch {
 @{message=[string]$_;stack=$_.ScriptStackTrace;no_cleanup_resubmitted=$true} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskOutput 'failure.json') -Encoding UTF8
 throw
}
