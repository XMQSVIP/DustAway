# Exercises deletion only in a unique, marked temporary workspace.
$ErrorActionPreference = 'Stop'
$testRoot = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) ('dustaway-clean-project-test-' + [guid]::NewGuid().ToString('N'))))
$cleanupSource = Join-Path $PSScriptRoot 'clean-project.ps1'
$knownLinks = New-Object 'System.Collections.Generic.List[string]'

function Write-FixtureFile([string]$Path, [string]$Value = 'generated') {
    New-Item -ItemType Directory -Path (Split-Path -Parent $Path) -Force | Out-Null
    [IO.File]::WriteAllText($Path, $Value)
}
function New-TestWorkspace([string]$Name) {
    $root = Join-Path $testRoot $Name
    Write-FixtureFile (Join-Path $root 'package.json') '{"name":"dustaway","version":"0.20.0"}'
    Write-FixtureFile (Join-Path $root 'Cargo.toml') '[workspace]'
    Write-FixtureFile (Join-Path $root 'scripts\placeholder.txt')
    Copy-Item -LiteralPath $cleanupSource -Destination (Join-Path $root 'scripts\clean-project.ps1')
    Write-FixtureFile (Join-Path $root 'src\keep.txt') 'source'
    Write-FixtureFile (Join-Path $root 'Cargo.lock') 'lockfile'
    Write-FixtureFile (Join-Path $root 'artifacts\portable\keep.exe') 'delivery'
    Write-FixtureFile (Join-Path $root 'artifacts\baseline-source\keep.txt') 'backup'
    Write-FixtureFile (Join-Path $root 'artifacts\evidence\keep.json') 'acceptance'
    Write-FixtureFile (Join-Path $root 'dist\index.html')
    return $root
}
function Assert-Exists([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { throw "Expected preserved path: $Path" }
}
function Assert-Absent([string]$Path) {
    if (Test-Path -LiteralPath $Path) { throw "Expected deleted path: $Path" }
}
function Add-TestJunction([string]$Path, [string]$Destination) {
    New-Item -ItemType Directory -Path (Split-Path -Parent $Path) -Force | Out-Null
    New-Item -ItemType Junction -Path $Path -Target $Destination | Out-Null
    $knownLinks.Add($Path)
}
function Assert-Rejected([scriptblock]$Action, [string]$ExpectedMessage) {
    $rejected = $false
    try { & $Action | Out-Null } catch {
        if ($_.Exception.Message -notmatch $ExpectedMessage) { throw }
        $rejected = $true
    }
    if (-not $rejected) { throw "Expected refusal: $ExpectedMessage" }
}

New-Item -ItemType Directory -Path $testRoot | Out-Null
Write-FixtureFile (Join-Path $testRoot '.cleanup-test-marker') 'dustaway-clean-project-script-test'
try {
    $workspace = New-TestWorkspace 'ordinary'
    $script = Join-Path $workspace 'scripts\clean-project.ps1'
    Write-FixtureFile (Join-Path $workspace 'target\debug\cache.bin')
    Write-FixtureFile (Join-Path $workspace 'artifacts\progress-visible-20261002\build-target\release\cache.lib')
    $readonlyCache = Join-Path $workspace 'target\debug\readonly-cache.bin'
    Write-FixtureFile $readonlyCache
    [IO.File]::SetAttributes($readonlyCache, [IO.FileAttributes]::ReadOnly)
    Write-FixtureFile (Join-Path $workspace 'target\release\app.exe')
    Write-FixtureFile (Join-Path $workspace 'node_modules\.vite\cache.json')
    Write-FixtureFile (Join-Path $workspace 'node_modules\dependency\keep.js')
    Write-FixtureFile (Join-Path $workspace 'artifacts\build.log')
    Write-FixtureFile (Join-Path $workspace 'artifacts\browser-validation-0.6\brave\browser.exe') 'test-browser'
    Write-FixtureFile (Join-Path $workspace 'artifacts\browser-validation-0.6\downloads\installer.exe') 'test-installer'
    Write-FixtureFile (Join-Path $workspace 'artifacts\browser-validation-0.6\evidence.json') 'browser-evidence'
    Write-FixtureFile (Join-Path $workspace 'artifacts\dustaway-analysis-bench-demo\.dustaway-test-only') 'readonly-analysis-benchmark'
    Write-FixtureFile (Join-Path $workspace 'artifacts\dustaway-analysis-bench-demo\sample.dat') 'fixture'
    Write-FixtureFile (Join-Path $workspace 'artifacts\dustaway-perf-demo\.dustaway-test-only') 'performance-fixture-only'
    Write-FixtureFile (Join-Path $workspace 'artifacts\dustaway-perf-demo\sample.tmp') 'fixture'
    Write-FixtureFile (Join-Path $workspace 'artifacts\evidence\dustaway-perf-home\.dustaway-test-only') 'performance-fixture-only'
    Write-FixtureFile (Join-Path $workspace 'artifacts\evidence\dustaway-perf-home\sample.tmp') 'fixture'
    Write-FixtureFile (Join-Path $workspace 'artifacts\evidence\scan-performance\dustaway-perf-home\.dustaway-test-only') 'performance-fixture-only'
    Write-FixtureFile (Join-Path $workspace 'artifacts\evidence\scan-performance\dustaway-perf-home\sample.tmp') 'fixture'
    Write-FixtureFile (Join-Path $workspace 'artifacts\evidence\scan-performance-isolated\dustaway-perf-home\.dustaway-test-only') 'performance-fixture-only'
    Write-FixtureFile (Join-Path $workspace 'artifacts\evidence\scan-performance-isolated\dustaway-perf-home\sample.tmp') 'fixture'
    Write-FixtureFile (Join-Path $workspace 'artifacts\dustaway-perf-bad\.dustaway-test-only') 'wrong-marker'
    Write-FixtureFile (Join-Path $workspace 'artifacts\dustaway-perf-bad\keep.tmp') 'not-a-confirmed-fixture'
    Write-FixtureFile (Join-Path $workspace 'artifacts\dustaway-e2e-ABC123\.dustaway-test-only') 'fixture-only'
    Write-FixtureFile (Join-Path $workspace 'artifacts\dustaway-e2e-ABC123\cache.bin')
    Write-FixtureFile (Join-Path $workspace 'artifacts\dustaway-e2e-NOMARK\keep.txt') 'not-marked'
    Write-FixtureFile (Join-Path $workspace 'artifacts\scoped_dir123_456\EBWebView\cache.bin')
    $report = & $script 3>$null | Out-String
    if ($report -notmatch 'Report only' -or $report -notmatch 'Selected:') { throw 'Invalid report' }
    & $script -Mode Clean -WhatIf 3>$null | Out-Null
    Assert-Exists (Join-Path $workspace 'dist\index.html')
    Assert-Exists (Join-Path $workspace 'target\debug\cache.bin')
    $measuredReport = & $script -MeasureSize 3>$null | Out-String
    if ($measuredReport -notmatch 'logical file sizes') { throw 'Full measurement was not performed' }
    & $script -Mode Clean -SkipE2EFixtures 3>$null | Out-Null
    Assert-Absent (Join-Path $workspace 'artifacts\progress-visible-20261002\build-target')
    Assert-Exists (Join-Path $workspace 'artifacts\dustaway-e2e-ABC123\cache.bin')
    & $script -Mode Clean 3>$null | Out-Null
    foreach ($relative in @('dist', 'target\debug', 'node_modules\.vite', 'artifacts\dustaway-e2e-ABC123', 'artifacts\scoped_dir123_456')) {
        Assert-Absent (Join-Path $workspace $relative)
    }
    foreach ($relative in @('target\release\app.exe', 'src\keep.txt', 'Cargo.lock', 'node_modules\dependency\keep.js', 'artifacts\portable\keep.exe', 'artifacts\baseline-source\keep.txt', 'artifacts\evidence\keep.json', 'artifacts\build.log', 'artifacts\dustaway-e2e-NOMARK\keep.txt', 'artifacts\dustaway-analysis-bench-demo\sample.dat')) {
        Assert-Exists (Join-Path $workspace $relative)
    }
    & $script -Mode Clean -RustScope All -IncludeLogs -IncludeBenchmarkFixtures 3>$null | Out-Null
    Assert-Absent (Join-Path $workspace 'target\release')
    Assert-Absent (Join-Path $workspace 'artifacts\build.log')
    Assert-Absent (Join-Path $workspace 'artifacts\dustaway-analysis-bench-demo')
    Assert-Absent (Join-Path $workspace 'artifacts\dustaway-perf-demo')
    Assert-Absent (Join-Path $workspace 'artifacts\evidence\dustaway-perf-home')
    Assert-Absent (Join-Path $workspace 'artifacts\evidence\scan-performance\dustaway-perf-home')
    Assert-Absent (Join-Path $workspace 'artifacts\evidence\scan-performance-isolated\dustaway-perf-home')
    Assert-Exists (Join-Path $workspace 'artifacts\evidence\keep.json')
    Assert-Exists (Join-Path $workspace 'artifacts\dustaway-perf-bad\keep.tmp')
    & $script -Mode Clean -RustScope None 3>$null | Out-Null
    Assert-Exists (Join-Path $workspace 'artifacts\browser-validation-0.6\brave\browser.exe')
    & $script -Mode Clean -RustScope None -IncludeBrowserArchives 3>$null | Out-Null
    Assert-Absent (Join-Path $workspace 'artifacts\browser-validation-0.6\downloads')
    Assert-Exists (Join-Path $workspace 'artifacts\browser-validation-0.6\brave\browser.exe')
    & $script -Mode Clean -RustScope None -IncludeBrowserTestAssets 3>$null | Out-Null
    Assert-Absent (Join-Path $workspace 'artifacts\browser-validation-0.6\brave')
    Assert-Exists (Join-Path $workspace 'artifacts\browser-validation-0.6\evidence.json')

    $protected = Join-Path $testRoot 'protected'
    Write-FixtureFile (Join-Path $protected 'sentinel.txt') 'do-not-delete'
    $deps = New-TestWorkspace 'dependencies'
    Write-FixtureFile (Join-Path $deps 'node_modules\.pnpm\pkg\index.js')
    Add-TestJunction (Join-Path $deps 'node_modules\linked-package') $protected
    & (Join-Path $deps 'scripts\clean-project.ps1') -Mode Clean -RustScope None -IncludeDependencies | Out-Null
    Assert-Absent (Join-Path $deps 'node_modules')
    if ([IO.File]::ReadAllText((Join-Path $protected 'sentinel.txt')) -ne 'do-not-delete') { throw 'Dependency link target changed' }

    $linked = New-TestWorkspace 'linked-target'
    Add-TestJunction (Join-Path $linked 'target') $protected
    & (Join-Path $linked 'scripts\clean-project.ps1') -Mode Clean | Out-Null
    Assert-Absent (Join-Path $linked 'dist')
    Assert-Exists (Join-Path $protected 'sentinel.txt')
    Assert-Exists (Join-Path $linked 'target')

    $nested = New-TestWorkspace 'nested-link'
    Add-TestJunction (Join-Path $nested 'dist\nested') $protected
    Assert-Rejected { & (Join-Path $nested 'scripts\clean-project.ps1') -Mode Clean -RustScope None } 'Refusing nested link'
    Assert-Exists (Join-Path $nested 'dist\index.html')

    $intermediate = New-TestWorkspace 'intermediate-link'
    Write-FixtureFile (Join-Path $protected '.vite\keep.txt') 'external-cache'
    Add-TestJunction (Join-Path $intermediate 'node_modules') $protected
    Assert-Rejected { & (Join-Path $intermediate 'scripts\clean-project.ps1') -Mode Clean -RustScope None } 'Refusing linked path or ancestor'
    Assert-Exists (Join-Path $intermediate 'dist\index.html')
    Assert-Exists (Join-Path $protected '.vite\keep.txt')

    $history = New-TestWorkspace 'historical-environments'
    foreach ($relative in @(
        'artifacts\portable\DustAway_0.20.0_windows-x64\app.exe',
        'artifacts\webdriver\driver.exe', 'artifacts\ssh-remote\ssh.exe',
        'artifacts\baseline-source.zip', 'artifacts\case\before.zip',
        'artifacts\case\evidence.json', 'artifacts\case\screenshot.png',
        'artifacts\historical-source\.git\objects\keep.exe',
        'artifacts\software-remote-acceptance\desktop-third-round\.git\index',
        'artifacts\software-remote-acceptance\desktop-third-round\keep-source.rs',
        'node_modules\dependency\index.js'
    )) {Write-FixtureFile (Join-Path $history $relative) 'preserved'}
    foreach ($relative in @(
        'artifacts\portable\DustAway_0.19.0_windows-x64\app.exe',
        'artifacts\home-cleanup-real-20261001\downloads\installer.exe',
        'artifacts\readonly-unlock-20261002\research\tool.exe',
        'artifacts\case\previous-test.exe', 'artifacts\case\previous-test.pdb',
        'artifacts\software-remote-acceptance\desktop-third-round\unused-test.exe',
        'artifacts\case\remote-bundle.zip'
    )) {Write-FixtureFile (Join-Path $history $relative) 'historical'}
    & (Join-Path $history 'scripts\clean-project.ps1') -Mode Clean -RustScope None -CurrentVersionOnly -SkipE2EFixtures | Out-Null
    foreach ($relative in @(
        'artifacts\portable\DustAway_0.20.0_windows-x64\app.exe',
        'artifacts\webdriver\driver.exe', 'artifacts\ssh-remote\ssh.exe',
        'artifacts\baseline-source.zip', 'artifacts\case\before.zip',
        'artifacts\case\evidence.json', 'artifacts\case\screenshot.png',
        'artifacts\historical-source\.git\objects\keep.exe',
        'artifacts\software-remote-acceptance\desktop-third-round\.git\index',
        'artifacts\software-remote-acceptance\desktop-third-round\keep-source.rs',
        'node_modules\dependency\index.js', 'src\keep.txt'
    )) {Assert-Exists (Join-Path $history $relative)}
    foreach ($relative in @(
        'artifacts\portable\DustAway_0.19.0_windows-x64',
        'artifacts\home-cleanup-real-20261001\downloads',
        'artifacts\readonly-unlock-20261002\research\tool.exe',
        'artifacts\case\previous-test.exe', 'artifacts\case\previous-test.pdb',
        'artifacts\software-remote-acceptance\desktop-third-round\unused-test.exe',
        'artifacts\case\remote-bundle.zip'
    )) {Assert-Absent (Join-Path $history $relative)}
    if (Get-Command git -ErrorAction SilentlyContinue) {
        $tracked = New-TestWorkspace 'tracked-files'
        & git -C $tracked init --quiet
        if ($LASTEXITCODE -ne 0) { throw 'Test git init failed' }
        & git -C $tracked add -- dist/index.html
        if ($LASTEXITCODE -ne 0) { throw 'Test git add failed' }
        Assert-Rejected { & (Join-Path $tracked 'scripts\clean-project.ps1') -Mode Clean -RustScope None } 'Preserving tracked file'
        Assert-Exists (Join-Path $tracked 'dist\index.html')
    }
    Write-Output 'PASS: preview, WhatIf, actual fixture deletion, preserved source/deliveries/evidence, optional logs/releases/dependencies/benchmark data, repeated cleanup, tracked-file protection, external cache exclusion, nested/intermediate link rejection.'
} finally {
    # Remove only links created by this test, then the verified marked test root.
    foreach ($link in $knownLinks) {
        $entry = Get-Item -LiteralPath $link -Force -ErrorAction SilentlyContinue
        if ($null -ne $entry -and ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            [IO.Directory]::Delete($link, $false)
        }
    }
    $tempBoundary = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (-not $testRoot.StartsWith($tempBoundary, [StringComparison]::OrdinalIgnoreCase) -or
        (Split-Path -Leaf $testRoot) -notmatch '^dustaway-clean-project-test-[a-f0-9]{32}$' -or
        [IO.File]::ReadAllText((Join-Path $testRoot '.cleanup-test-marker')) -ne 'dustaway-clean-project-script-test') {
        throw 'Refusing to remove an unrecognized test directory.'
    }
    [void][DustAwayProjectCleanupWalker]::Scan($testRoot, $false)
    Remove-Item -LiteralPath $testRoot -Recurse -Force
}
