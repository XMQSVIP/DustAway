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
  if(($i.Attributes-band [IO.FileAttributes]::ReparsePoint)-ne 0){throw 'Protection does not follow links'}
  if($i.PSIsContainer){$queue.Enqueue($i.FullName)}elseif(!$skip.ContainsKey((Normalize $i.FullName).ToLowerInvariant())){$rows.Add(@{path=$i.FullName;bytes=$i.Length;sha256=(Get-FileHash -LiteralPath $i.FullName).Hash})}
 }}
 return @($rows | Sort-Object {$_.path})
}
function Verify-Protected($Rows){foreach($r in $Rows){if(!(Test-Path -LiteralPath $r.path)-or(Get-FileHash -LiteralPath $r.path).Hash-ne $r.sha256){throw "Protected file changed: $($r.path)"}}}
$driver=Start-Process -FilePath (Join-Path $taskRoot 'bin/tauri-driver.exe') -ArgumentList @('--native-driver',(Join-Path $taskRoot 'artifacts/webdriver/msedgedriver.exe')) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $taskOutput 'driver.log') -RedirectStandardError (Join-Path $taskOutput 'driver-error.log')
try {
 Await {Driver GET '/status' $null} | Out-Null
 $script:session=(Driver POST '/session' @{capabilities=@{alwaysMatch=@{'tauri:options'=@{application=$taskExe}}}}).sessionId
 $boot=Await {Ipc 'bootstrap'}
 if($boot.capabilities.fixture -or $boot.capabilities.system_cleanup){throw 'Not portable production build'}
 & ([scriptblock]::Create([IO.File]::ReadAllText((Join-Path $taskRoot 'scripts/window-state.ps1')))) -Executable $taskExe -Width 1100 -Height 780 | Out-Null
 $rules=@(
  @{id='dev.pip_http';root=(Join-Path $env:LOCALAPPDATA 'pip/Cache')},
  @{id='dev.pip_wheels';root=(Join-Path $env:LOCALAPPDATA 'pip/Cache/wheels')},
  @{id='dev.npm_cache';root=(Join-Path $env:LOCALAPPDATA 'npm-cache/_cacache')}
 )
 foreach($rule in $rules){
  Click '//button[starts-with(normalize-space(.),"开始扫描")]'
  $scan=Await {$s=Ipc 'get_runtime_snapshot' @{scope='user'};if($s.status.state-eq'ready'){$s}}
  $row=$scan.summary.rules | Where-Object rule_id -eq $rule.id
  Save-Evidence ($rule.id+'-scan.json') @{sha256=$taskHash;summary=$row;session=$scan.status.id}
  if(!$row.files-or$row.blocked-or$row.phase-ne'completed'){
   Save-Evidence ($rule.id+'-blocked.json') @{reason='No complete selectable real candidates';summary=$row}
   # No mutation: reset to home through a new application session to keep next case independent.
   Driver DELETE "/session/$script:session" $null | Out-Null
   $script:session=(Driver POST '/session' @{capabilities=@{alwaysMatch=@{'tauri:options'=@{application=$taskExe}}}}).sessionId
   Await {Ipc 'bootstrap'} | Out-Null
   continue
  }
  $candidates=@();for($offset=0;$offset-lt$row.files;$offset+=200){$page=Ipc 'list_candidates' @{scope='user';session=$scan.status.id;rule=$rule.id;offset=$offset;limit=200};$candidates+=@($page.items)}
  $allowed=(Normalize $rule.root)+'\'
  foreach($c in $candidates){if($c.rule_id-ne$rule.id-or!(Normalize $c.path).StartsWith($allowed,[StringComparison]::OrdinalIgnoreCase)){throw 'Candidate outside authorized cache'}}
  $protected=@(Protect $rule.root $candidates)
  foreach($p in @('D:\Python312\python.exe','D:\nodejs\node.exe',(Join-Path $env:USERPROFILE '.local/bin/uv.exe'))){if(Test-Path -LiteralPath $p){$protected+=@{path=$p;bytes=(Get-Item -LiteralPath $p).Length;sha256=(Get-FileHash -LiteralPath $p).Hash}}}
  foreach($p in @('D:\Python312\Lib\site-packages\pip','D:\nodejs\node_modules\npm')){$protected+=@(Protect $p @())}
  Save-Evidence ($rule.id+'-candidates.json') $candidates
  Save-Evidence ($rule.id+'-protected.json') $protected
  Js 'document.querySelectorAll(".review-workspace details").forEach(d=>d.open=true);document.querySelectorAll(".purpose-toggle[aria-expanded=false]").forEach(b=>b.click());' | Out-Null
  foreach($id in @(Js 'return [...document.querySelectorAll(".rule-row input:checked")].map(e=>e.id);')){Click "//input[@id='$id']"}
  Click ("//input[@id='choose-"+$rule.id+"']")
  Click '//button[starts-with(normalize-space(.),"清理已选")]'
  Await {Js 'return !!document.querySelector("[role=alertdialog]");'} | Out-Null
  $receipt=(Ipc 'get_runtime_snapshot' @{scope='user'}).control.last_request
  if($receipt.command-ne'prepare_cleanup'-or$receipt.error){throw 'No valid native plan'}
  $plans=@($receipt.value)
  if($plans.Count-ne 1-or$plans[0].plan.files-ne$candidates.Count-or@($plans[0].plan.rule_ids).Count-ne 1-or$plans[0].plan.rule_ids[0]-ne$rule.id){throw 'Plan outside authorized rule'}
  Capture ($rule.id+'-confirmation.png')
  Verify-Protected $protected
  Save-Evidence ($rule.id+'-started.json') @{sha256=$taskHash;plans=$plans;automatic_replay=$false}
  Click '//*[@role="alertdialog"]//button[normalize-space(.)="永久删除"]'
  $report=Await {$s=Ipc 'get_runtime_snapshot' @{scope='user'};if($s.report-and$s.status.state-ne'cleaning'){$s.report}}
  Save-Evidence ($rule.id+'-report.json') $report
  Verify-Protected $protected
  if($report.deleted-ne$candidates.Count-or($report.failed+$report.skipped+$report.unknown)-ne 0){throw 'Incomplete cleanup'}
  foreach($c in $candidates){if(Test-Path -LiteralPath $c.path){throw 'Reported deletion still exists'}}
  Save-Evidence ($rule.id+'-verified.json') @{sha256=$taskHash;protected_unchanged=$true;protected_count=$protected.Count;deleted=$report.deleted;bytes=$report.deleted_bytes}
  Capture ($rule.id+'-completed.png')
  Click '//button[normalize-space(.)="完成"]'
 }
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
