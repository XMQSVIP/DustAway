param([Parameter(Mandatory=$true)][string]$RunRoot)
$ErrorActionPreference='Stop'
$root=$RunRoot
$utf8=[Text.UTF8Encoding]::new($false)
function Write-Atomic($name,$value){
  $path=Join-Path $root $name
  $temp=$path+'.tmp'
  [IO.File]::WriteAllText($temp,($value|ConvertTo-Json -Depth 20),$utf8)
  Move-Item -LiteralPath $temp -Destination $path -Force
}
function Now { [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() }
$request=Get-Content -LiteralPath (Join-Path $root 'request.json') -Raw | ConvertFrom-Json
$state=[ordered]@{schema=3;runId=$request.runId;host=$env:COMPUTERNAME;ordinaryUser=$false;status='running';stages=@();realDeletionAttempted=$false;webpageRecoveryVerified=$false}
try {
  if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $request.schema -ne 3 -or $request.version -ne '0.24.0'){throw 'Wrong environment'}
  if($root -cne ('C:\Users\MSN\AppData\Local\DustAwayAcceptance\coverage-024\'+$request.runId)){throw 'Wrong run directory'}
  if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test')) -cne ('DustAway.Coverage.0.24.0:'+$request.runId)){throw 'Wrong marker'}
  $dispatch=Get-Content -LiteralPath (Join-Path $root 'started.json') -Raw | ConvertFrom-Json
  if($dispatch.runId -cne $request.runId -or (Get-FileHash -LiteralPath (Join-Path $root 'request.json')).Hash -ine $dispatch.requestSha256){throw 'Dispatched request changed'}
  $identity=[Security.Principal.WindowsIdentity]::GetCurrent()
  if($identity.Name -ine 'DESKTOP-24NJTT6\MSN' -or ([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary MSN token required'}
  $state.ordinaryUser=$true
  foreach($name in @('windows-tests.exe','core-tests.exe','probe.exe','worker.ps1')){
    if((Get-Item -LiteralPath (Join-Path $root $name)).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Linked input'}
    if((Get-FileHash -LiteralPath (Join-Path $root $name)).Hash -ine $request.hashes.$name){throw 'Input changed'}
  }
  $env:DUSTAWAY_COVERAGE_RUN=$request.runId
  $env:DUSTAWAY_ACCEPTANCE_CANCEL=Join-Path $root 'cancel.request'
  Remove-Item Env:DUSTAWAY_WEIXIN_EXECUTE,Env:DUSTAWAY_WEIXIN_TEST_MODE -ErrorAction SilentlyContinue
  $steps=@(
    @{id='preflight';exe='probe.exe';args=@()},
    @{id='boundaries';exe='windows-tests.exe';args=@('--exact','platform::scan_tests::coverage_observed::observed_rules_cover_only_fixed_leaves_and_refuse_all_plans','--nocapture')},
    @{id='states';exe='windows-tests.exe';args=@('--exact','platform::scan_tests::coverage_observed::observed_rules_distinguish_empty_missing_denied_cancelled_and_excluded','--nocapture')},
    @{id='links';exe='windows-tests.exe';args=@('--exact','platform::scan_tests::coverage_observed::observed_files_count_hard_links_once_and_do_not_follow_junctions','--nocapture')},
    @{id='core';exe='core-tests.exe';args=@('--test-threads=1')}
  )
  Write-Atomic 'status.json' $state
  foreach($step in $steps){
    if(Test-Path -LiteralPath $env:DUSTAWAY_ACCEPTANCE_CANCEL){$state.status='cancelled';break}
    if((Get-FileHash -LiteralPath (Join-Path $root $step.exe)).Hash -ine $request.hashes.($step.exe)){throw 'Stage binary changed'}
    if($step.rule){$env:DUSTAWAY_WEIXIN_RULE=$step.rule}
    $started=Now
    # Intent is persisted before each test process; Start never replays an existing run.
    $intent=[ordered]@{runId=$request.runId;stage=$step.id;startedAt=$started;binarySha256=$request.hashes.($step.exe)}
    $stream=[IO.File]::Open((Join-Path $root ($step.id+'.intent.json')),[IO.FileMode]::CreateNew)
    try{$bytes=$utf8.GetBytes(($intent|ConvertTo-Json));$stream.Write($bytes,0,$bytes.Length);$stream.Flush($true)}finally{$stream.Dispose()}
    $launch=@{FilePath=(Join-Path $root $step.exe);WorkingDirectory=$root;WindowStyle='Hidden';PassThru=$true;RedirectStandardOutput=(Join-Path $root ($step.id+'.log'));RedirectStandardError=(Join-Path $root ($step.id+'.err'))}
    if($step.args.Count){$launch.ArgumentList=$step.args}
    try{$process=Start-Process @launch}catch{
      foreach($suffix in @('.log','.err')){$path=Join-Path $root ($step.id+$suffix);if(-not(Test-Path -LiteralPath $path)){[IO.File]::WriteAllText($path,'',$utf8)}}
      $state.stages+=@{id=$step.id;startedAt=$started;finishedAt=(Now);exitCode=$null;status='failed';failureKind='process_start_failed'}
      $state.status='failed';Write-Atomic 'status.json' $state;break
    }
    $null=$process.Handle
    $timer=[Diagnostics.Stopwatch]::StartNew();$cancelled=$false
    while(-not $process.WaitForExit(200)){
      if($timer.Elapsed.TotalSeconds -ge 120 -or (Test-Path -LiteralPath $env:DUSTAWAY_ACCEPTANCE_CANCEL)){
        [IO.File]::WriteAllText($env:DUSTAWAY_ACCEPTANCE_CANCEL,'cancel requested',$utf8)
        $cancelled=$true
        break
      }
    }
    $phaseState='passed';$code=$null
    if($cancelled -and -not $process.WaitForExit(30000)){
      $phaseState='uncertain'
      # This handle belongs only to the test process launched above; never target an application by name.
      $process.Kill();$process.WaitForExit()
    }else{
      $process.WaitForExit();$code=$process.ExitCode
      if($cancelled){$phaseState='cancelled'}elseif($code -ne 0){$phaseState='failed'}
    }
    $phase=@{id=$step.id;startedAt=$started;finishedAt=(Now);exitCode=$code;status=$phaseState}
    if($phaseState -eq 'failed'){$phase.failureKind='test_process_nonzero_exit'}
    $state.stages+=$phase
    if($phaseState -ne 'passed'){$state.status=$phaseState;Write-Atomic 'status.json' $state;break}
    Write-Atomic 'status.json' $state
  }
  if($state.status -eq 'running'){$state.status='passed'}
}catch{
  $state.status='failed'
  # Avoid serializing arbitrary exception messages that may contain account paths.
  [IO.File]::WriteAllText((Join-Path $root 'worker.err'),'Test orchestration failed; inspect stage/exit evidence.',$utf8)
}finally{
  Write-Atomic 'status.json' $state
}
if($state.status -ne 'passed'){exit 1}
