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
$state=[ordered]@{schema=2;runId=$request.runId;host=$env:COMPUTERNAME;ordinaryUser=$false;status='running';stages=@();realDeletionAttempted=$false;webpageRecoveryVerified=$false}
try {
  if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $request.schema -ne 2 -or $request.version -ne '0.21.1'){throw 'Wrong environment'}
  if($root -cne ('C:\Users\MSN\AppData\Local\DustAwayAcceptance\weixin-0211\'+$request.runId)){throw 'Wrong run directory'}
  if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test')) -cne ('DustAway.Weixin.0.21.1:'+$request.runId)){throw 'Wrong marker'}
  $dispatch=Get-Content -LiteralPath (Join-Path $root 'started.json') -Raw | ConvertFrom-Json
  if($dispatch.runId -cne $request.runId -or (Get-FileHash -LiteralPath (Join-Path $root 'request.json')).Hash -ine $dispatch.requestSha256){throw 'Dispatched request changed'}
  $identity=[Security.Principal.WindowsIdentity]::GetCurrent()
  if($identity.Name -ine 'DESKTOP-24NJTT6\MSN' -or ([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary MSN token required'}
  $state.ordinaryUser=$true
  foreach($name in @('weixin-tests.exe','core-tests.exe','probe.exe','worker.ps1')){
    if((Get-Item -LiteralPath (Join-Path $root $name)).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Linked input'}
    if((Get-FileHash -LiteralPath (Join-Path $root $name)).Hash -ine $request.hashes.$name){throw 'Input changed'}
  }
  $env:DUSTAWAY_WEIXIN_RUN=$request.runId
  $env:DUSTAWAY_ACCEPTANCE_CANCEL=Join-Path $root 'cancel.request'
  Remove-Item Env:DUSTAWAY_WEIXIN_EXECUTE,Env:DUSTAWAY_WEIXIN_TEST_MODE -ErrorAction SilentlyContinue
  $steps=@(
    @{id='preflight';exe='probe.exe';args=@()},
    @{id='code';exe='weixin-tests.exe';args=@('--exact','weixin_cache::acceptance::production_weixin_engine_acceptance','--ignored','--nocapture');rule='weixin.code'},
    @{id='gpu';exe='weixin-tests.exe';args=@('--exact','weixin_cache::acceptance::production_weixin_engine_acceptance','--ignored','--nocapture');rule='weixin.gpu'},
    @{id='fixtures';exe='weixin-tests.exe';args=@('--exact','platform::application_acceptance_tests::weixin_cache_cleanup_receipts','--nocapture')}
  )
  $cases=@(
    'weixin_frozen_selection_rejects_equal_count_replacements_and_identity_changes',
    'all_six_rules_reject_unsafe_files_and_same_version_identity_replacement',
    'each_rule_cancels_remaining_files_and_recovery_never_resumes_deletion',
    'each_rule_preserves_excluded_new_and_busy_files_outside_the_frozen_plan',
    'each_rule_rechecks_changes_between_two_deletions',
    'each_rule_reports_partial_reads_and_never_cleans_unreadable_subtrees',
    'eligibility_never_spreads_and_long_term_exclusions_prevent_discovery',
    'five_cache_rules_refuse_replaced_roots_and_junctions_after_confirmation',
    'rules_recheck_version_process_identity_marker_and_changed_files',
    'six_rule_cleanup_receipts',
    'weixin_age_boundary_uses_full_snapshot_validation'
  )
  for($i=0;$i -lt $cases.Count;$i++){$steps+=@{id=('boundary-'+$i);exe='weixin-tests.exe';args=@('--exact',('platform::application_acceptance_tests::'+$cases[$i]),'--nocapture')}}
  $steps+=@{id='attachments';exe='core-tests.exe';args=@('attachments::tests::','--test-threads=1','--nocapture')}
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
