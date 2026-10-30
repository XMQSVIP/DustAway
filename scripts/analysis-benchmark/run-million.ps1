param(
    [string]$Output = 'artifacts/analysis-million/formal-r3',
    [string]$IndexDirectory = 'D:\DustAway-tests\dustaway-analysis-cache-million-20261002',
    [string]$Baseline = 'target/bench-million-baseline/release/dustaway-analysis-bench.exe',
    [string]$Current = 'target/bench-million/release/dustaway-analysis-bench.exe',
    [string]$SmallFixture = 'artifacts/dustaway-analysis-bench-0.10',
    [string]$LargeFixture = 'F:\DustAway-tests\dustaway-analysis-bench-million-v1',
    [switch]$MatchedCurrentBaseline
)
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Set-Location -LiteralPath $workspace
$env:DUSTAWAY_BENCH_INDEX_ROOT = $IndexDirectory
$destination = [IO.Path]::GetFullPath((Join-Path $workspace $Output))
if (Test-Path -LiteralPath $destination) { throw 'Choose a new output folder; raw results must not be overwritten.' }
New-Item -ItemType Directory -Path $destination | Out-Null
$old = (Resolve-Path -LiteralPath $Baseline).Path
$new = (Resolve-Path -LiteralPath $Current).Path
$fixture = (Resolve-Path -LiteralPath $SmallFixture).Path
$large = (Resolve-Path -LiteralPath $LargeFixture).Path
$hashes = @{baseline=(Get-FileHash -LiteralPath $old).Hash; current=(Get-FileHash -LiteralPath $new).Hash}
$environment = @{started_at=(Get-Date).ToString('o'); system=(Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,BuildNumber,TotalVisibleMemorySize); cpu=(Get-CimInstance Win32_Processor | Select-Object Name); hashes=$hashes; cache_directory=$IndexDirectory; matched_current_baseline=[bool]$MatchedCurrentBaseline; method='Same host, Release, sequential old/new runs; no cache flushing. Paths, binary hashes and compiler counts are recorded. With MatchedCurrentBaseline both programs use the same disk-index implementation and comparison limit; otherwise this is the historical memory-index baseline.'}
$environment | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $destination 'environment.json') -Encoding utf8
function Measure-Case([string]$Name,[string]$Binary,[string]$Root,[string]$Count,[string[]]$Extra=@()) {
    $started=Get-Date
    @{started_at=$started.ToString('o'); ambient_compilers=@(Get-Process rustc,link,clippy-driver -ErrorAction SilentlyContinue).Count; available_memory_kib=(Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $destination "$Name-load.json") -Encoding utf8
    Write-Output "Starting $Name at $($started.ToString('o'))"
    $raw=& $Binary $Root $Count @Extra
    if ($LASTEXITCODE -ne 0) { throw "Benchmark failed: $Name" }
    $result=$raw | ConvertFrom-Json
    if (-not $result.state) { throw "Missing result: $Name" }
    $raw | Set-Content -LiteralPath (Join-Path $destination "$Name.json") -Encoding utf8
    Write-Output "$Name state=$($result.state) files=$($result.files) elapsed_ms=$($result.elapsed_ms) peak_private_bytes=$($result.peak_private_bytes)"
    if ([int64]$Count -ge 1000000 -and $Extra.Count -eq 0 -and ($result.state -ne 'completed' -or $result.files -ne [int64]$Count)) { throw "Large scan did not complete: $Name" }
}
Measure-Case 'current-1000000-1' $new $large '1000000'
Measure-Case 'current-3000000-1' $new $large '3000000'
foreach ($count in @('100000','500000')) {
    foreach ($round in 1..5) {
        if ($MatchedCurrentBaseline) {
            Measure-Case "baseline-$count-$round" $old $fixture $count @('--comparison')
        } else {
            Measure-Case "baseline-$count-$round" $old $fixture $count
        }
        Measure-Case "current-$count-$round" $new $fixture $count @('--comparison')
    }
}
foreach ($round in 2..3) {
    Measure-Case "current-1000000-$round" $new $large '1000000'
    Measure-Case "current-3000000-$round" $new $large '3000000'
}
Measure-Case 'cancel-scan' $new $large '1000000' @('--cancel')
Measure-Case 'cancel-query-during-scan' $new $large '1000000' @('--cancel-query')
Measure-Case 'cancel-index' $new $fixture '100000' @('--cancel-index')
if ((Get-FileHash -LiteralPath $old).Hash -ne $hashes.baseline -or (Get-FileHash -LiteralPath $new).Hash -ne $hashes.current) { throw 'Binaries changed during measurement.' }
Write-Output "Finished at $((Get-Date).ToString('o'))"
