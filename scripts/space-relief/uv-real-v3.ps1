param([string]$TestRoot)
# Production WebDriver acceptance without Node/Python processes that would block npm/pip.
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($TestRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $env:USERNAME -ne 'MSN'){throw 'Wrong host/user'}
if([IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-space-relief')) -ne 'DustAway.SpaceRelief.20261002'){throw 'Wrong marker'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Limited user required'}
$taskOutput=$env:DUSTAWAY_TEST_OUTPUT
$taskExe=$env:DUSTAWAY_TEST_EXECUTABLE
$taskHash=(Get-FileHash -LiteralPath $taskExe).Hash
function Save-Evidence($Name,$Value){
 $p=Join-Path $taskOutput $Name
 if(Test-Path -LiteralPath $p){throw "Evidence already exists: $Name"}
 [IO.File]::WriteAllText($p,($Value | ConvertTo-Json -Depth 35),[Text.UTF8Encoding]::new($false))
}
function Driver($Method,$Path,$Body){
 $args=@{Method=$Method;Uri=('http://127.0.0.1:4444'+$Path);TimeoutSec=180}
 if($null -ne $Body){$args.ContentType='application/json; charset=utf-8';$args.Body=[Text.Encoding]::UTF8.GetBytes(($Body | ConvertTo-Json -Depth 30 -Compress))}
 $r=Invoke-RestMethod @args
 if($r.value -isnot [array] -and $null -ne $r.value -and $r.value.PSObject.Properties['error'] -and $r.value.error){throw ($r.value | ConvertTo-Json -Compress)}
 return $r.value
}
function Js([string]$Code,[object[]]$Arguments=@()){Driver POST "/session/$script:session/execute/sync" @{script=$Code;args=@($Arguments)}}
function Ipc([string]$Name,$Arguments=@{}){Js 'return window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]);' @($Name,$Arguments)}
function Await($Block){$end=(Get-Date).AddSeconds(180);do{try{$v=& $Block;if($v){return $v}}catch{$last=$_};Start-Sleep -Milliseconds 200}while((Get-Date)-lt $end);throw "Timed out: $last"}
function Click([string]$XPath){
 $id=Await {foreach($r in @(Driver POST "/session/$script:session/elements" @{using='xpath';value=$XPath})){$id=$r.'element-6066-11e4-a52e-4f735466cecf';if((Driver GET "/session/$script:session/element/$id/displayed" $null)-and(Driver GET "/session/$script:session/element/$id/enabled" $null)){return $id}}}
 Driver POST "/session/$script:session/element/$id/click" @{} | Out-Null
}
function Capture($Name){$b=Driver GET "/session/$script:session/screenshot" $null;[IO.File]::WriteAllBytes((Join-Path $taskOutput $Name),[Convert]::FromBase64String($b))}
function Normalize($Path){[IO.Path]::GetFullPath(($Path -replace '^\\\\\?\\','')).TrimEnd('\')}
function Protect($Root,$Candidates){
 $skip=@{};foreach($c in $Candidates){$skip[(Normalize $c.path).ToLowerInvariant()]=$true}
 $rows=New-Object Collections.Generic.List[object]
 $queue=New-Object Collections.Generic.Queue[string];if(Test-Path -LiteralPath $Root){$queue.Enqueue($Root)}
 while($queue.Count){foreach($i in Get-ChildItem -LiteralPath $queue.Dequeue() -Force){
  if(($i.Attributes-band [IO.FileAttributes]::ReparsePoint)-ne 0){$rows.Add(@{path=$i.FullName;kind='link';target=[string]$i.Target;link_type=[string]$i.LinkType;attributes=[string]$i.Attributes;modified=$i.LastWriteTimeUtc.Ticks});continue}
  if($i.PSIsContainer){$queue.Enqueue($i.FullName)}elseif(!$skip.ContainsKey((Normalize $i.FullName).ToLowerInvariant())){$rows.Add(@{path=$i.FullName;bytes=$i.Length;sha256=(Get-FileHash -LiteralPath $i.FullName).Hash})}
 }}
 return @($rows | Sort-Object {$_.path})
}
function Verify-Protected($Rows){foreach($r in $Rows){
 if($r.kind-eq'link'){$i=Get-Item -LiteralPath $r.path -Force;if([string]$i.Target-ne$r.target-or[string]$i.LinkType-ne$r.link_type-or[string]$i.Attributes-ne$r.attributes-or$i.LastWriteTimeUtc.Ticks-ne$r.modified){throw "Protected link changed: $($r.path)"}}
 elseif(!(Test-Path -LiteralPath $r.path)-or(Get-FileHash -LiteralPath $r.path).Hash-ne$r.sha256){throw "Protected file changed: $($r.path)"}
}}
$driver=Start-Process -FilePath (Join-Path $taskRoot 'bin/tauri-driver.exe') -ArgumentList @('--native-driver',(Join-Path $taskRoot 'artifacts/webdriver/msedgedriver.exe')) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $taskOutput 'driver.log') -RedirectStandardError (Join-Path $taskOutput 'driver-error.log')
try {
 Await {Driver GET '/status' $null} | Out-Null
 $script:session=(Driver POST '/session' @{capabilities=@{alwaysMatch=@{'tauri:options'=@{application=$taskExe}}}}).sessionId
 $boot=Await {Ipc 'bootstrap'}
 if($boot.capabilities.fixture -or $boot.capabilities.system_cleanup){throw 'Not portable production build'}
 & ([scriptblock]::Create([IO.File]::ReadAllText((Join-Path $taskRoot 'scripts/window-state.ps1')))) -Executable $taskExe -Width 1100 -Height 780 | Out-Null
 # uv uses only its official command and existing production preflight.
 Click '//button[starts-with(normalize-space(.),"开始扫描")]'
 $scan=Await {$s=Ipc 'get_runtime_snapshot' @{scope='user'};if($s.status.state-eq'ready'){$s}}
 Save-Evidence 'real-summary.json' $scan.summary
 Save-Evidence 'application-support.json' (Ipc 'get_application_support')
 $testEnvironment=Join-Path $taskOutput 'protected-project-env'
 if(Test-Path -LiteralPath $testEnvironment){throw 'Do not reuse environment fixture'}
 & 'D:\Python312\python.exe' -m venv --without-pip $testEnvironment
 if($LASTEXITCODE-ne 0){throw 'Failed to prepare protected project environment'}
 $protected=@();foreach($p in @('D:\Python312\python.exe','D:\nodejs\node.exe',(Join-Path $env:USERPROFILE '.local/bin/uv.exe'))){if(Test-Path -LiteralPath $p){$protected+=@{path=$p;sha256=(Get-FileHash -LiteralPath $p).Hash}}}
 foreach($p in @((Join-Path $env:APPDATA 'uv/python'),(Join-Path $env:APPDATA 'uv/tools'))){$protected+=@(Protect $p @())}
 $protected+=@(Protect $testEnvironment @())
 Save-Evidence 'uv-protected.json' $protected
 Click '//button[normalize-space(.)="整理 uv 闲置缓存"]'
 Await {Js 'return !!document.querySelector(".uv-dialog");'} | Out-Null
 $plan=(Ipc 'get_runtime_snapshot' @{scope='user'}).control.last_request.value
 $expected=Normalize (Join-Path $env:LOCALAPPDATA 'uv/cache')
 if((Normalize $plan.cache)-ne$expected){throw 'uv cache outside reviewed real root'}

 Capture 'uv-real-confirmation.png'
 Save-Evidence 'uv-real-started.json' @{sha256=$taskHash;plan=$plan;automatic_replay=$false}
 Click '//*[@role="dialog"]//button[normalize-space(.)="确认整理闲置缓存"]'
 $report=Await {$r=Ipc 'get_uv_task';if($r-and$r.id-eq$plan.id-and$r.status-ne'running'){$r}}
 Save-Evidence 'uv-real-report.json' $report
 Verify-Protected $protected
 & (Join-Path $testEnvironment 'Scripts/python.exe') -c 'import sys; assert sys.prefix != sys.base_prefix; print(sys.prefix)' *> (Join-Path $taskOutput 'uv-environment-check.log')
 if($LASTEXITCODE-ne 0){throw 'Protected Python environment no longer works'}
 Save-Evidence 'uv-real-verified.json' @{sha256=$taskHash;protected_unchanged=$true;protected_count=$protected.Count;status=$report.status}
 Capture 'uv-real-completed.png'
 if($report.status-ne'completed'){throw 'uv did not complete; retain evidence, never replay'}
 & 'D:\Python312\python.exe' -m pip --version *> (Join-Path $taskOutput 'pip-version.log')
 if($LASTEXITCODE-ne 0){throw 'pip functional check failed'}
 & 'D:\nodejs\node.exe' 'D:\nodejs\node_modules\npm\bin\npm-cli.js' --version *> (Join-Path $taskOutput 'npm-version.log')
 if($LASTEXITCODE-ne 0){throw 'npm functional check failed'}
 Save-Evidence 'complete.json' @{sha256=$taskHash;completed=$true;ordinary_user=$true}
} catch {
 Save-Evidence 'failure.json' @{message=[string]$_;stack=$_.ScriptStackTrace;sha256=$taskHash}
 if($script:session){try{Capture 'failure.png'}catch{}}
 throw
} finally {
 if($script:session){try{Driver DELETE "/session/$script:session" $null | Out-Null}catch{}}
 # Only our test driver, never an application under test.
 if(!$driver.HasExited){Stop-Process -Id $driver.Id}
}
