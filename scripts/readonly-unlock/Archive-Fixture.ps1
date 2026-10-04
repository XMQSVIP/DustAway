# Preserve the verified synthetic application files outside the standard application directory.
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20261002-readonly-unlock-v1'))
if($env:COMPUTERNAME-ne'DESKTOP-24NJTT6'-or[IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-readonly-unlock'))-ne'DustAway.ReadonlyUnlock.20261002.V1'){throw 'Wrong environment'}
$taskRun=Get-Content -LiteralPath (Join-Path $taskRoot 'evidence/matrix/run.json') -Raw | ConvertFrom-Json
if($taskRun.exit_code-ne0){throw 'Preserve failed fixture in place'}
if(Get-Process -Name dustaway,tauri-driver,msedgedriver,HBuilderX -ErrorAction SilentlyContinue){throw 'Wait for desktop processes to exit normally'}
$taskSource=[IO.Path]::GetFullPath((Join-Path $env:APPDATA 'HBuilder X'))
$taskDestination=[IO.Path]::GetFullPath((Join-Path $taskRoot 'evidence/matrix/archived-hbuilder-fixture'))
$taskFixture=Get-Content -LiteralPath (Join-Path $taskRoot 'evidence/matrix/application-fixture.json') -Raw | ConvertFrom-Json
if($taskFixture.root-ne$taskSource-or!$taskDestination.StartsWith($taskRoot+'\',[StringComparison]::OrdinalIgnoreCase)-or(Test-Path -LiteralPath $taskDestination)){throw 'Unexpected source or archive location'}
$taskFiles=@(Get-ChildItem -LiteralPath $taskSource -Force -Recurse)
if(@(@(Get-Item -LiteralPath $taskSource)+$taskFiles | Where-Object {($_.Attributes-band[IO.FileAttributes]::ReparsePoint)-ne0}).Count){throw 'Do not move linked content'}
$taskExpected=@($taskFixture.protected | Where-Object {$_.path.StartsWith($taskSource+'\',[StringComparison]::OrdinalIgnoreCase)})
if($taskExpected.Count-ne4-or@($taskFiles | Where-Object {!$_.PSIsContainer}).Count-ne4-or@($taskFiles | Where-Object {$_.PSIsContainer}).Count-ne3){throw 'Application data changed; preserve in place'}
foreach($taskFile in $taskExpected){if((Get-FileHash -LiteralPath $taskFile.path).Hash-ne$taskFile.sha256){throw 'Fixture changed; preserve in place'}}
Move-Item -LiteralPath $taskSource -Destination $taskDestination
foreach($taskFile in $taskExpected){$taskArchived=Join-Path $taskDestination $taskFile.path.Substring($taskSource.Length+1);if((Get-FileHash -LiteralPath $taskArchived).Hash-ne$taskFile.sha256){throw 'Archive hash mismatch'}}
@{source=$taskSource;archive=$taskDestination;files=$taskExpected;verified=$true} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $taskRoot 'evidence/matrix/fixture-archived.json') -Encoding UTF8
